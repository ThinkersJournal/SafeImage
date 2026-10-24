// SPDX-License-Identifier: MIT OR Apache-2.0
// Reference harness for the SafeImage PDQ fixtures. This file is original SafeImage code
// (MIT OR Apache-2.0). It links the unmodified upstream PDQ sources at the pinned commit and
// prints, for each raw RGB frame file given, the values that tests/reference.rs compares.
//
// usage: harness <frames_dir> <name> <w> <h> [<name> <w> <h> ...]
// Each frame is read from <frames_dir>/<name>.rgb (packed RGB8, row-major).

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include <pdq/cpp/downscaling/downscaling.h>
#include <pdq/cpp/hashing/pdqhashing.h>
#include <pdq/cpp/hashing/torben.h>

using facebook::pdq::hashing::Hash256;

namespace facebook {
namespace pdq {
namespace hashing {
// Declared here in case the upstream header does not export it; defined in pdqhashing.cpp.
void fillFloatLumaFromRGB(
    uint8_t* pRbase, uint8_t* pGbase, uint8_t* pBbase, int numRows, int numCols,
    int rowStride, int colStride, float* luma);
}  // namespace hashing
}  // namespace pdq
}  // namespace facebook

static uint64_t fnv_bytes(const void* p, size_t n) {
  uint64_t h = 1469598103934665603ULL;
  const uint8_t* b = static_cast<const uint8_t*>(p);
  for (size_t i = 0; i < n; i++) {
    h ^= b[i];
    h *= 1099511628211ULL;
  }
  return h;
}

int main(int argc, char** argv) {
  if (argc < 5 || (argc - 2) % 3 != 0) {
    std::fprintf(stderr, "usage: harness <dir> <name> <w> <h> ...\n");
    return 2;
  }
  std::string dir = argv[1];

  // dct64To16 on a fixed non-trivial 64x64 input (the matrix itself is file-local upstream).
  {
    static float a[64][64], t[16][64], b[16][16];
    for (int i = 0; i < 64; i++)
      for (int j = 0; j < 64; j++)
        a[i][j] = (float)(((i * 131 + j * 71 + (i ^ j) * 17) % 255)) / 3.0f;
    facebook::pdq::hashing::dct64To16(a, t, b);
    std::printf("dct64to16 %016llx", (unsigned long long)fnv_bytes(b, sizeof(b)));
    for (int k = 0; k < 4; k++) std::printf(" %08x", *reinterpret_cast<uint32_t*>(&b[0][k]));
    std::printf("\n");
  }

  // torben on the same arrays tests/support::torben_cases() builds (formulas must match).
  {
    auto emit = [](const char* name, std::vector<float> v) {
      float m = facebook::pdq::hashing::torben(v.data(), (int)v.size());
      uint32_t bits;
      std::memcpy(&bits, &m, 4);
      std::printf("torben %s %08x\n", name, bits);
    };
    std::vector<float> ties(256), flat(256, 7.0f), halves(256), mixed(256), odd(255);
    for (int i = 0; i < 256; i++) ties[i] = (float)(i % 16);
    for (int i = 0; i < 256; i++) halves[i] = i < 128 ? 0.0f : 1.0f;
    for (size_t i = 0; i < 256; i++)
      mixed[i] = (float)(((i * 2654435761ULL) >> 7) % 1000) / 8.0f - 20.0f;
    for (int i = 0; i < 255; i++) odd[i] = (float)((i * 37) % 101);
    emit("ties", ties);
    emit("flat", flat);
    emit("halves", halves);
    emit("mixed", mixed);
    emit("odd", odd);
  }

  for (int a = 2; a + 2 < argc + 0; a += 3) {
    std::string name = argv[a];
    int w = std::atoi(argv[a + 1]);
    int h = std::atoi(argv[a + 2]);
    std::string path = dir + "/" + name + ".rgb";
    FILE* f = std::fopen(path.c_str(), "rb");
    if (!f) {
      std::fprintf(stderr, "cannot open %s\n", path.c_str());
      return 1;
    }
    std::vector<uint8_t> rgb((size_t)w * h * 3);
    if (std::fread(rgb.data(), 1, rgb.size(), f) != rgb.size()) {
      std::fprintf(stderr, "short read %s\n", path.c_str());
      return 1;
    }
    std::fclose(f);

    size_t n = (size_t)w * h;
    std::vector<float> luma_h(n), luma_d(n);
    // Both upstream definitions of the RGB-to-luma step, on the same input.
    facebook::pdq::hashing::fillFloatLumaFromRGB(
        rgb.data(), rgb.data() + 1, rgb.data() + 2, h, w, 3 * w, 3, luma_h.data());
    facebook::pdq::downscaling::fillFloatLumaFromRGB(
        rgb.data(), rgb.data() + 1, rgb.data() + 2, h, w, 3 * w, 3, luma_d.data());
    uint64_t fh = fnv_bytes(luma_h.data(), n * 4);
    uint64_t fd = fnv_bytes(luma_d.data(), n * 4);

    std::printf("frame %s %d %d\n", name.c_str(), w, h);
    std::printf("luma_hashing %016llx\n", (unsigned long long)fh);
    std::printf("luma_downscaling %016llx\n", (unsigned long long)fd);

    // Single hash.
    {
      std::vector<float> b1 = luma_h, b2(n);
      static float b64[64][64], b16x64[16][64], b16[16][16];
      Hash256 hash;
      int quality = -1;
      facebook::pdq::hashing::pdqHash256FromFloatLuma(
          b1.data(), b2.data(), h, w, b64, b16x64, b16, hash, quality);
      std::printf("pdq %s %d\n", hash.format().c_str(), quality);
    }
    // Dihedral hashes.
    {
      std::vector<float> b1 = luma_h, b2(n);
      static float b64[64][64], b16x64[16][64], b16[16][16], b16aux[16][16];
      Hash256 hs[8];
      int quality = -1;
      facebook::pdq::hashing::pdqDihedralHash256esFromFloatLuma(
          b1.data(), b2.data(), h, w, b64, b16x64, b16, b16aux, &hs[0], &hs[1], &hs[2],
          &hs[3], &hs[4], &hs[5], &hs[6], &hs[7], quality);
      std::printf("dihedral_quality %d\n", quality);
      for (int i = 0; i < 8; i++) std::printf("dihedral %d %s\n", i, hs[i].format().c_str());
    }
  }
  return 0;
}
