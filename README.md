# daytape

## Installation

1. Clone this repo.
2. `cd daytape && cargo install --path .`
3. Add this to your `.tmux.conf` file: `set -g status-right '#(daytape)'`

You should now see `No task set` in your tmux bar. Once you've set a schedule for today, it will show the current and upcoming events instead.

## Usage

Use `daytape edit` to interactively edit the day's schedule.

Controls for this editor are:

- Use the arrow keys to move the cursor.
- Start typing to create a calendar item.
- Use the `[` and `]` keys to decrease/increase the duration of the calendar item by 5 minutes.
- Use the `:` key followed by another character to execute a command:
    - `:s` - Save.
    - `:q` - Save and quit.
    - `:d` - Delete selected calendar item.
    - `:x` - Quit without saving.

## Notes

- Daytape only retains the schedule for today and tomorrow at most. Older entries are deleted.
