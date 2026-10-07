use crate::data::Reader;
use crate::prelude::*;

/// 数据生成器参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

/// 数据生成器
pub trait Generator: Send {
    /// 使生成器就绪（如编译源码）；须先于 [`Generator::run`] 调用
    ///
    /// # Errors
    ///
    /// 编译选项解析失败、编译器进程无法启动、源文件拷贝或依赖写入失败，或编译以非零码
    /// 退出（消息携带编译错误输出）时返回 `Err`。
    fn prepare(&mut self) -> Result<()>;

    /// 运行生成器，返回生成的输入流
    ///
    /// `args` 为该数据点的生成参数，`seed` 为随机种子。
    ///
    /// # Errors
    ///
    /// 未先经 [`Generator::prepare`] 使生成器就绪、生成器进程无法启动、读写产出的输出流
    /// 失败，或生成器以非零状态退出（`Err` 携带 stderr）时返回 `Err`。
    fn run(&self, args: IndexMap<String, Arg>, seed: u64) -> Result<Box<dyn Reader>>;
}

/// Checker（SPJ）结果类型
#[derive(Debug, Clone, PartialEq)]
pub enum JudgeResult {
    Accepted,
    WrongAnswer,
    /// 格式错误（按答案错误处理）
    PresentationError,
    /// 校验器自身失败，无法判定
    Fail,
    /// 按 0-100 给出的部分分
    Score(f64),
}

/// Checker（SPJ）
pub trait Checker: Send {
    /// 使 checker 就绪（如编译源码）；须先于 [`Checker::validate`] 调用
    ///
    /// # Errors
    ///
    /// 编译选项解析失败、编译器进程无法启动、源文件拷贝或依赖写入失败，或编译以非零码
    /// 退出（消息携带编译错误输出）时返回 `Err`。
    fn prepare(&mut self) -> Result<()>;

    /// 校验选手输出：对照输入与标准答案判定
    ///
    /// 返回判定结果与 checker 的说明文本（可为空串）。
    ///
    /// # Errors
    ///
    /// checker 无法完成判定时返回 `Err`：未先 prepare、输入/输出/答案落盘失败、
    /// 进程无法启动，或结果报告文件缺失、无法解析。
    fn validate(
        &self,
        input: &mut dyn Reader,
        output: &mut dyn Reader,
        answer: &mut dyn Reader,
    ) -> Result<(JudgeResult, String)>;
}

/// Validator（输入校验器）结果
#[derive(Debug, Clone, PartialEq)]
pub enum ValidatorResult {
    /// 校验通过
    Ok,
    /// 校验失败，附带 stderr 信息
    Invalid(String),
}

/// Validator（输入校验器）
pub trait Validator: Send {
    /// 使 validator 就绪（如编译源码）；须先于 [`Validator::validate`] 调用
    ///
    /// # Errors
    ///
    /// 编译选项解析失败、编译器进程无法启动、源文件拷贝或依赖写入失败，或编译以非零码
    /// 退出（消息携带编译错误输出）时返回 `Err`。
    fn prepare(&mut self) -> Result<()>;

    /// 校验输入文件
    ///
    /// # Errors
    ///
    /// validator 无法完成判定时返回 `Err`：未先 prepare、输入落盘失败或进程无法启动；
    /// 输入合法与否经 [`ValidatorResult`] 返回，因此 validator 以非零码退出不算 `Err`。
    fn validate(&self, input: &mut dyn Reader) -> Result<ValidatorResult>;
}
