// 这是一个示例标准程序（题解），请根据自己的题目逻辑改写，题目名称请自行替换。
//
// 想使用它：在 conf.json 的 "tests" 中加入
//   "std": { "expected": "== 100", "path": "tests/std.cpp" }
// 详见 https://docs.tuack-ng.ink/app/test/config.html

#include <cstdio>

int main()
{
    freopen("p.in", "r", stdin);
    freopen("p.out", "w", stdout);

    long long n, m;
    scanf("%lld%lld", &n, &m);
    printf("%lld\n", n + m);
    return 0;
}
