use tempfile::NamedTempFile;

use crate::prelude::*;
use quick_xml::de::from_str;
use tuack_lib::data::Reader;
pub use tuack_lib::utils::testlib::JudgeResult;

#[derive(Debug, Deserialize, PartialEq)]
struct XmlResult {
    #[serde(rename = "@outcome")]
    outcome: String,
    #[serde(rename = "@pctype", default)]
    pctype: Option<String>,
    #[serde(rename = "@points", default)]
    points: Option<String>,
    #[serde(rename = "$text")]
    text: Option<String>,
}

/// 解析 testlib checker 的 XML 结果，返回 [`JudgeResult`] 与消息文本；解析成功的分数会被裁剪
/// 到 `[0, 100]`。
///
/// # Errors
///
/// XML 解析失败、`outcome` 取值未知，或 `partially-correct`/`points` 的分数缺失或无法
/// 解析为数字时返回 `Err`。
pub fn parse_result(xml_str: &str) -> Result<(JudgeResult, String)> {
    let xml_str = xml_str.trim();
    let xml_result: XmlResult = from_str(xml_str)?;

    let message = xml_result.text.unwrap_or_default();

    let result = match xml_result.outcome.as_str() {
        "accepted" => JudgeResult::Accepted,
        "wrong-answer" => JudgeResult::WrongAnswer,
        "presentation-error" => JudgeResult::PresentationError,
        "fail" => JudgeResult::Fail,
        "partially-correct" => {
            let score = parse_score_value(&xml_result.pctype)?;
            JudgeResult::Score(score)
        }
        "points" => {
            let score = parse_score_value(&xml_result.points)?;
            JudgeResult::Score(score)
        }
        other => bail!("Unknown outcome type: {}", other),
    };

    Ok((result, message))
}

/// 将输入流流式写入临时文件，供 SPJ 程序按路径读取。
///
/// # Errors
///
/// 创建临时文件或写入内容失败时返回 `Err`。
pub fn write_temp(reader: &mut dyn Reader, prefix: &str) -> Result<NamedTempFile> {
    let mut tmp = NamedTempFile::with_prefix(prefix)?;
    std::io::copy(reader, &mut tmp)?;
    Ok(tmp)
}

fn parse_score_value(attr_value: &Option<String>) -> Result<f64> {
    if let Some(value_str) = attr_value {
        return value_str
            .parse::<f64>()
            .map(|score| score.clamp(0.0, 100.0))
            .with_context(|| format!("分数解析失败：'{}'", value_str));
    }
    bail!("缺失分数字段");
}
