/// 当前工作目录对应的配置层级位置
#[derive(Debug, Clone)]
pub enum CurrentLocation {
    /// 不属于任何配置文件
    None,
    /// 配置文件根目录
    Root,
    /// 比赛日配置文件，参数为比赛日目录名
    Day(String),
    /// 题目配置文件，参数为比赛日目录名与题目目录名
    Problem(String, String),
}
