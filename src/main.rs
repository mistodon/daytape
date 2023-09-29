use chrono::Timelike;
use clap::Parser;
use color_eyre::eyre::Result;
use directories::ProjectDirs;
use termbuffer::Color;

use daytape::{DayState, Schedule, Task, Time, TimeSlot};

/// A whole day's schedule in your terminal
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    sub: Option<SubCommand>,

    #[command(flatten)]
    show_args: ShowArgs,
}

#[derive(Debug, Parser)]
enum SubCommand {
    /// Show the schedule (default behaviour)
    Show {
        #[command(flatten)]
        show_args: ShowArgs,
    },

    /// Edit the schedule
    Edit {
        /// Edit tomorrow's schedule instead of today's
        #[arg(long)]
        tomorrow: bool,
    },
}

#[derive(Parser, Debug)]
struct ShowArgs {
    /// Show tomorrow's schedule instead of today's
    #[arg(long)]
    tomorrow: bool,

    /// The number of characters to display in the day tape
    #[arg(short, long, default_value_t = 48)]
    width: u32,
}

const COLORS: &[[u8; 3]] = &[
    [190, 0, 0],
    [0, 190, 0],
    [15, 52, 215],
    [227, 159, 0],
    [0, 190, 190],
    [156, 0, 190],
    [0, 176, 123],
    [104, 0, 176],
    [255, 0, 172],
    [0, 122, 198],
];

fn get_color_index(label: &str) -> usize {
    let digest = md5::compute(label.as_bytes());
    let bytes: [u8; 16] = digest.into();
    let number: u128 = u128::from_le_bytes(bytes);
    (number % COLORS.len() as u128) as usize
}

fn get_edit_color(index: usize) -> Color {
    let [r, g, b] = COLORS[index % COLORS.len()];
    Color::Rgb(r, g, b)
}

fn get_tmux_color(index: usize) -> String {
    let [r, g, b] = COLORS[index % COLORS.len()];
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn get_dirs() -> ProjectDirs {
    let dirs = ProjectDirs::from("com", "mistodon", "daytape").unwrap();
    std::fs::create_dir_all(dirs.cache_dir()).unwrap();
    std::fs::create_dir_all(dirs.config_dir()).unwrap();
    dirs
}

fn target_date(now: chrono::DateTime<chrono::Local>, tomorrow: bool) -> chrono::NaiveDate {
    let tomorrow = tomorrow || now.hour() > 18;
    let offset = match tomorrow {
        true => 1,
        _ => 0,
    };
    (now + chrono::Duration::days(offset)).date_naive()
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.sub {
        Some(SubCommand::Edit { tomorrow }) => edit(tomorrow),
        Some(SubCommand::Show { show_args }) => tmux(&show_args),
        None => tmux(&args.show_args),
    }
}

fn load_schedule(path: &std::path::Path) -> Result<Schedule> {
    let source = std::fs::read_to_string(path)?;
    let schedule: Schedule = serde_yaml::from_str(&source)?;
    Ok(schedule)
}

fn tmux(show_args: &ShowArgs) -> Result<()> {
    let now = chrono::Local::now();
    let target_date = target_date(now, show_args.tomorrow);

    let dirs = get_dirs();
    let mut main_file = dirs.config_dir().to_owned();
    main_file.push("daytape.yaml");

    let schedule: Schedule = load_schedule(&main_file).unwrap_or(Schedule::default());
    let state = schedule.dates.get(&target_date);

    let width = show_args.width as usize;

    if state.is_none() {
        print!(
            "#[bg=red]{: <width$}#[bg=default]",
            "No task set",
            width = width
        );
        return Ok(());
    }
    let state = state.unwrap();

    let mut to_display = String::new();

    let mut task = None;
    let mut run = 0;

    let mut time = Time::new(now.hour() as usize, now.minute() as usize);
    for _ in 0..width {
        use std::fmt::Write;

        let current_task = state.tasks.iter().find(|task| task.slot.contains(time));
        if current_task == task {
            run += 1;
        } else {
            task = current_task;
            run = 0;
            let color = match task {
                Some(task) => get_tmux_color(get_color_index(&task.label)),
                None => "default".to_owned(),
            };
            write!(&mut to_display, "#[bg={color}]").unwrap();
        }

        let ch = task
            .and_then(|task| task.label.chars().nth(run))
            .unwrap_or(' ');
        write!(&mut to_display, "{ch}").unwrap();

        time += Time::mins(1);
    }

    print!("{to_display}#[bg=default]");

    Ok(())
}

fn edit(tomorrow: bool) -> Result<()> {
    use std::time::{Duration, Instant};
    use termbuffer::{char, App, Draw, Event, Key};

    let solid_text_color = Color::Rgb(240, 240, 240);
    let text_color = solid_text_color;
    let sel_color = Color::Rgb(190, 150, 255);

    let now = chrono::Local::now();
    let today = now.date_naive();
    let target_date = target_date(now, tomorrow);

    let dirs = get_dirs();
    let mut main_file = dirs.config_dir().to_owned();
    main_file.push("daytape.yaml");

    let mut schedule: Schedule = load_schedule(&main_file).unwrap_or(Schedule::default());

    let delay = Duration::from_millis(1000 / 60);

    let mut app = App::builder().build().unwrap();

    let mut state = schedule
        .dates
        .get(&target_date)
        .cloned()
        .unwrap_or_else(|| DayState {
            date: target_date.clone(),
            tasks: vec![],
        });

    let mut cursor: Time = Time::new(7, 0);

    fn drawtext(d: &mut Draw, text: &str, from: [usize; 2], max_x: usize, fg: Color, bg: Color) {
        let [x, y] = from;
        for (i, ch) in text.chars().enumerate() {
            let x = x + i;
            if x > max_x {
                break;
            }
            d.set(y, x, char!(ch, fg, bg));
        }
    }

    let mut typed = String::new();
    let mut cmd_mode = false;

    loop {
        let mut quit = false;
        let mut save = false;
        let mut scale_up = false;
        let mut scale_down = false;
        let mut delete = false;
        let mut backspace = false;

        let start_at = Instant::now();

        let selected_slot = state
            .tasks
            .iter()
            .map(|task| task.slot)
            .find(|slot| slot.contains(cursor));

        const VALID_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_,.;'\"/|()0123456789!?<>~+=*&^%$#@ ";

        for event in app.events() {
            match event.unwrap() {
                Event::Key(Key::Left) => match selected_slot {
                    Some(slot) => cursor = slot.start - Time::mins(5),
                    None => cursor -= Time::mins(5),
                },
                Event::Key(Key::Right) => match selected_slot {
                    Some(slot) => cursor = slot.end(),
                    None => cursor += Time::mins(5),
                },
                Event::Key(Key::Up) => cursor -= Time::hours(1),
                Event::Key(Key::Down) => cursor += Time::hours(1),
                Event::Key(Key::Backspace) => backspace = true,
                Event::Key(Key::Esc) => cmd_mode = false,
                Event::Key(Key::Char(c)) if cmd_mode => {
                    cmd_mode = false;
                    match c {
                        'q' => {
                            save = true;
                            quit = true;
                        }
                        's' => save = true,
                        'd' => delete = true,
                        'x' => quit = true,
                        _ => (),
                    }
                }
                Event::Key(Key::Char(c)) => match c {
                    '[' => scale_down = true,
                    ']' => scale_up = true,
                    ':' => cmd_mode = !cmd_mode,
                    ' ' => typed.push(' '),
                    ch if VALID_CHARS.contains(ch) => typed.push(ch),
                    _ => (),
                },
                _ => (),
            }
        }

        cursor = cursor.clamp(Time::new(7, 0), Time::new(18, 55));

        if save {
            schedule.dates.retain(|date, _| date >= &today);
            schedule.dates.insert(target_date, state.clone());
            let output = serde_yaml::to_string(&schedule)?;
            std::fs::write(&main_file, &output)?;
        }

        if quit {
            return Ok(());
        }

        if delete {
            state.tasks.retain(|task| !task.slot.contains(cursor));
        }

        let create_if_empty = !typed.is_empty();
        if create_if_empty && !state.tasks.iter().any(|task| task.slot.contains(cursor)) {
            state.tasks.push(Task {
                slot: TimeSlot {
                    start: cursor,
                    duration: 15,
                },
                label: String::new(),
            });
            state.tasks.sort();
        }

        let selected_task = state
            .tasks
            .iter_mut()
            .find(|task| task.slot.contains(cursor));
        if let Some(task) = selected_task {
            task.label.push_str(&typed);
            typed.clear();

            if backspace {
                task.label.pop();
            }
            if scale_up {
                task.slot.duration += 5;
            }
            if scale_down {
                if task.slot.duration > 5 {
                    task.slot.duration -= 5;
                }
            }
            if scale_up || scale_down {
                cursor = task.slot.end() - Time::mins(5);
            }
        }

        {
            let mut draw = app.draw();
            let draw = &mut draw;
            let [_w, _h] = [draw.columns(), draw.rows()];
            let date = target_date.to_string();
            drawtext(draw, &date, [0, 0], 10, text_color, Color::Default);

            let text_color = match cmd_mode {
                false => text_color,
                true => Color::Rgb(140, 140, 140),
            };

            drawtext(
                draw,
                "time ||  .  .  |  .  .  |  .  .  |  .  .  |",
                [0, 1],
                99,
                text_color,
                Color::Default,
            );
            for (i, hour) in (7..=18).enumerate() {
                drawtext(
                    draw,
                    &format!("{hour: >4} |"),
                    [0, i + 2],
                    6,
                    text_color,
                    Color::Default,
                );
            }

            let [ox, oy] = [6, 2];
            let [cx, cy] = cursor.to_grid();
            let cx = ox + cx * 3;
            let cy = oy + cy - 7;

            draw.set(cy, cx, char!(' ', Color::Default, sel_color));

            let max_width = 36;
            for task in &state.tasks {
                let [x, y] = task.slot.start.to_grid();
                let mut x = ox + x * 3;
                let mut y = oy + y - 7;
                let mut label_width = (task.slot.duration / 5) * 3;

                while label_width > 0 {
                    let usable_width = std::cmp::min(label_width, max_width - (x - ox));
                    label_width -= usable_width;

                    let label = format!("{: <1$}", &task.label, usable_width);

                    let color = get_edit_color(get_color_index(&task.label));
                    let color = if task.slot.contains(cursor) {
                        sel_color
                    } else {
                        color
                    };
                    drawtext(
                        draw,
                        &label,
                        [x, y],
                        x + usable_width - 1,
                        Color::Rgb(240, 240, 240),
                        color,
                    );
                    x = ox;
                    y += 1;
                }
            }

            if cmd_mode {
                const DOCS: &str = ": (s)ave | save&(q)uit | e(x)it | (d)elete";
                drawtext(draw, DOCS, [0, 14], 99, solid_text_color, Color::Default);
            }
        }

        let end_at = Instant::now();
        if end_at < start_at + delay {
            std::thread::sleep(delay - (end_at - start_at));
        }
    }
}
