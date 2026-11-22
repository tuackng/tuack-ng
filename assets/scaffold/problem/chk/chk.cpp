// 这是示例 SPJ，请根据自己的题目逻辑改写。
//
// 想使用它：在 conf.json 中配置 "checker"：
//   "checker": {
//     "data": { "source": "chk/chk.cpp", "deps": ["deps/testlib.h"] }
//   }
// 不配置时使用默认的全文比较。
// 详见 https://docs.tuack-ng.ink/app/test/spj.html

#include "testlib.h"

int main(int argc, char* argv[])
{
    registerTestlibCmd(argc, argv);

    long long n = inf.readLong();
    long long m = inf.readLong();
    long long out = ouf.readLong();

    if (out != n + m)
        quitf(_wa, "n + m = %lld，实际输出 %lld", n + m, out);
    quitf(_ok, "答案正确");
}
