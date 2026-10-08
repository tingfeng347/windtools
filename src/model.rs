use anyhow::{Result, bail};
use chrono::{DateTime, Local, LocalResult, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderState {
    None,
    Pending,
    Sending,
    Sent,
    Missed,
    Failed,
    Cancelled,
}

impl ReminderState {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "未设置提醒",
            Self::Pending => "等待提醒",
            Self::Sending => "正在提交",
            Self::Sent => "已提交系统",
            Self::Missed => "提醒已错过",
            Self::Failed => "发送失败",
            Self::Cancelled => "提醒已取消",
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pending => "pending",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Missed => "missed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "none" => Ok(Self::None),
            "pending" => Ok(Self::Pending),
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "missed" => Ok(Self::Missed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => bail!("未知提醒状态：{value}"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub note: String,
    pub completed: bool,
    pub remind_at: Option<i64>,
    pub reminder_state: ReminderState,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub revision: i64,
}

impl Task {
    pub fn new(title: &str, note: &str, remind_at: Option<i64>, now: i64) -> Result<Self> {
        validate_text(title, note)?;
        if remind_at.is_some_and(|at| at <= now) {
            bail!("提醒时间必须在未来");
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            title: title.trim().to_owned(),
            note: note.to_owned(),
            completed: false,
            remind_at,
            reminder_state: if remind_at.is_some() {
                ReminderState::Pending
            } else {
                ReminderState::None
            },
            last_error: None,
            created_at: now,
            updated_at: now,
            revision: 0,
        })
    }
    pub fn validate(&self) -> Result<()> {
        Uuid::parse_str(&self.id)?;
        if self.revision < 0 {
            bail!("无效事项版本");
        }
        validate_text(&self.title, &self.note)?;
        if self.last_error.as_ref().is_some_and(|error| {
            error.len() > 16_384 || error.chars().any(|c| c.is_control() && c != '\n')
        }) {
            bail!("无效提醒错误信息");
        }
        for value in [Some(self.created_at), Some(self.updated_at), self.remind_at]
            .into_iter()
            .flatten()
        {
            if DateTime::from_timestamp(value, 0).is_none() {
                bail!("日期超出可表示范围");
            }
        }
        if self.remind_at.is_none() && self.reminder_state != ReminderState::None {
            bail!("提醒状态与时间不一致");
        }
        if self.remind_at.is_some() && self.reminder_state == ReminderState::None {
            bail!("提醒状态与时间不一致");
        }
        if self.completed
            && matches!(
                self.reminder_state,
                ReminderState::Pending | ReminderState::Sending
            )
        {
            bail!("已完成事项不能有待发提醒");
        }
        Ok(())
    }
}

pub fn validate_text(title: &str, note: &str) -> Result<()> {
    if title.trim().is_empty() {
        bail!("请输入事项标题");
    }
    if title.chars().count() > 512 {
        bail!("标题最多 512 个字符");
    }
    if note.chars().count() > 8_000 {
        bail!("备注最多 8000 个字符");
    }
    if title.chars().any(char::is_control) || note.chars().any(|c| c.is_control() && c != '\n') {
        bail!("内容包含不支持的控制字符");
    }
    Ok(())
}

pub fn parse_reminder(input: &str, now: i64) -> Result<Option<i64>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    let at = if let Some(relative) = input.strip_prefix('+') {
        let boundary = relative
            .char_indices()
            .last()
            .map(|(index, _)| index)
            .unwrap_or(0);
        let (number, unit) = relative.split_at(boundary);
        let count: i64 = number
            .parse()
            .map_err(|_| anyhow::anyhow!("相对时间示例：+10m、+2h、+1d"))?;
        let seconds = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86400,
            _ => bail!("时间单位使用 s、m、h 或 d"),
        };
        now.checked_add(
            count
                .checked_mul(seconds)
                .ok_or_else(|| anyhow::anyhow!("时间过大"))?,
        )
        .ok_or_else(|| anyhow::anyhow!("时间过大"))?
    } else if let Ok(explicit) = DateTime::parse_from_rfc3339(input) {
        explicit.timestamp()
    } else {
        let naive = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M")
            .or_else(|_| NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S"))
            .map_err(|_| anyhow::anyhow!("使用 YYYY-MM-DD HH:MM，或 +10m；留空取消提醒"))?;
        match Local.from_local_datetime(&naive) {
            LocalResult::Single(value) => value.timestamp(),
            LocalResult::Ambiguous(_, _) => {
                bail!("此本地时间有两种时区解释，请输入带偏移的时间，如 2026-11-01T01:30:00-04:00")
            }
            LocalResult::None => bail!("此本地时间不存在，请选择其他时间"),
        }
    };
    if at <= now {
        bail!("提醒时间已过去，请设置未来时间");
    }
    if DateTime::from_timestamp(at, 0).is_none() {
        bail!("日期超出可表示范围");
    }
    Ok(Some(at))
}

pub fn local_time(at: i64, format: &str) -> String {
    DateTime::from_timestamp(at, 0)
        .map(|value| value.with_timezone(&Local).format(format).to_string())
        .unwrap_or_else(|| "无效日期".into())
}

pub fn ambiguous_reminder_candidates(input: &str) -> Option<[i64; 2]> {
    let naive = NaiveDateTime::parse_from_str(input.trim(), "%Y-%m-%d %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(input.trim(), "%Y-%m-%d %H:%M:%S"))
        .ok()?;
    match Local.from_local_datetime(&naive) {
        LocalResult::Ambiguous(first, second) => Some([first.timestamp(), second.timestamp()]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reminders_reject_past_and_bad_input() {
        assert_eq!(parse_reminder("+2m", 100).unwrap(), Some(220));
        assert_eq!(parse_reminder("", 100).unwrap(), None);
        for input in ["+0m", "+-1m", "+", "+10分", "bad", "+999999999999999999d"] {
            assert!(parse_reminder(input, 100).is_err(), "{input}");
        }
        assert!(Task::new("\u{1b}title", "", None, 100).is_err());
    }
    #[test]
    fn explicit_timezone_is_unambiguous() {
        assert_eq!(
            parse_reminder("2030-01-01T10:00:00+08:00", 100).unwrap(),
            Some(1893463200)
        );
    }
}
