// 这是示例 validator，请根据自己的题目逻辑改写。
//
// 想使用它：在 conf.json 中配置 "validator"：
//   "validator": {
//     "data": { "source": "val/val.cpp", "deps": ["deps/testlib.h"] }
//   }
// 可在 "generator" 的 "data" 中加上 "validate": true，让 dmk 生成后自动校验。
// 详见 https://docs.tuack-ng.ink/app/validate/spec.html

#include "testlib.h"

int main(int argc, char* argv[]) {
  registerValidation(argc, argv);

  inf.readInt(1, 100000, "n");
  inf.readSpace();
  inf.readInt(1, 1000000000, "m");
  inf.readEoln();
  inf.readEof();
  return 0;
}
