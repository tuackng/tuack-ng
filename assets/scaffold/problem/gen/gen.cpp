// 这是示例生成器，请根据自己的题目逻辑改写。
//
// 想使用它：
//   1. 把 conf.json 中的 "dmk" 改为 "on"；
//   2. 在 conf.json 中配置 "generator"：
//      "generator": {
//        "data": { "gen": "gen/gen.cpp", "deps": ["deps/testlib.h"] }
//      }
// 详见 https://docs.tuack-ng.ink/app/dmk/generator.html

#include "testlib.h"

int main(int argc, char* argv[])
{
    registerGen(argc, argv, 1);

    rnd.setSeed(opt<unsigned long long>("seed"));
    int n = rnd.next(1, opt<int>("n"));
    int m = rnd.next(1, opt<int>("m"));

    printf("%d %d\n", n, m);
    return 0;
}
