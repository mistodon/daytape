use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Time {
    pub hour: usize,
    pub min: usize,
}

impl Time {
    pub const fn new(hour: usize, min: usize) -> Self {
        Time { hour, min }
    }

    pub const fn hours(hours: usize) -> Self {
        Time {
            hour: hours,
            min: 0,
        }
    }

    pub const fn mins(mins: usize) -> Self {
        Time { hour: 0, min: mins }
    }

    pub const fn to_grid(&self) -> [usize; 2] {
        let y = self.hour;
        let x = self.min / 5;
        [x, y]
    }
}

impl Add<Time> for Time {
    type Output = Time;

    fn add(self, other: Time) -> Self::Output {
        let min = self.min + other.min;
        let plus_hours = min / 60;
        let hour = (self.hour + other.hour + plus_hours) % 24;
        let min = min % 60;
        Time { hour, min }
    }
}

impl AddAssign<Time> for Time {
    fn add_assign(&mut self, other: Time) {
        *self = *self + other;
    }
}

impl Sub<Time> for Time {
    type Output = Time;

    fn sub(self, other: Time) -> Self::Output {
        let (min, minus_hours) = if self.min >= other.min {
            (self.min - other.min, 0)
        } else {
            ((self.min + 60) - other.min, 1)
        };

        let hour = ((self.hour + 24) - other.hour - minus_hours) % 24;

        Time { hour, min }
    }
}

impl SubAssign<Time> for Time {
    fn sub_assign(&mut self, other: Time) {
        *self = *self - other;
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TimeSlot {
    pub start: Time,
    pub duration: usize,
}

impl TimeSlot {
    pub fn end(&self) -> Time {
        self.start + Time::mins(self.duration)
    }

    pub fn contains(&self, time: Time) -> bool {
        self.start <= time && time < self.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub slot: TimeSlot,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DayState {
    pub date: String,
    pub tasks: Vec<Task>,
}
