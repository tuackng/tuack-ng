use crate::prelude::*;

/// 语言配置：编译与运行该语言提交所需的命令
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Language {
    /// 语言名称
    pub language: String,
    /// 编译器，为 `None` 时跳过编译
    pub compiler: Option<Compiler>,
    /// 运行器，为 `None` 时直接运行
    pub runner: Option<Runner>,
}

/// 编译器配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Compiler {
    /// 可执行文件名，如 `g++`、`javac`
    pub executable: String,
    /// 版本检查命令模板，如 `"{executable} --version"`
    pub check: String,
    /// 编译命令模板
    ///
    /// 如 `"{executable} -o {output_path}/{program_name}{exe_suffix} {args} {input_path}"`、
    /// `"{executable} -d {output_path}/ {args} {input_path}"`。
    ///
    /// 可用变量：
    /// - `{executable}`：同 `executable`
    /// - `{output_path}`：输出目录
    /// - `{program_name}`：这道题叫啥（也是预期文件名）
    /// - `{args}`：该语言的编译选项
    /// - `{input_path}`：源文件路径
    /// - `{exe_suffix}`：exe 后缀名
    pub run: String,
}

/// 运行器配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Runner {
    /// 可执行文件名，如 `python`、`java`
    pub executable: String,
    /// 版本检查命令模板，如 `"{executable} --version"`
    pub check: String,
    /// 运行命令模板
    ///
    /// 如 `"{executable} {input_path}/{program_name}.py"`；CP 约定下假定选手使用 `Main`，
    /// 如 `"{executable} {input_path}/Main.class"`。
    ///
    /// 可用变量：
    /// - `{executable}`：同 `executable`
    /// - `{input_path}`：编译器产物路径（上一步的 `output_path`），跳过编译时会直接拷贝源文件（保留后缀但名字变成这道题）
    /// - `{program_name}`：这道题叫啥（也是预期文件名）
    /// - `{exe_suffix}`：exe 后缀名
    pub run: String,
}
