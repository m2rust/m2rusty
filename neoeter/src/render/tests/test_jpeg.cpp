#include <support/TestAssert.h>

#include <cstdio>
#include <cstdlib>
#include <vector>

#include <Neoeter/Platform/Base/ImageLoaderCompat.h>
#include <Neoeter/Render/JpegFile.h>

static bool ReadFile(const char* path, std::vector<unsigned char>& out)
{
	FILE* file = std::fopen(path, "rb");
	if (!file)
		return false;

	std::fseek(file, 0, SEEK_END);
	long size = std::ftell(file);
	std::fseek(file, 0, SEEK_SET);
	if (size <= 0)
	{
		std::fclose(file);
		return false;
	}

	out.resize((size_t) size);
	size_t read = std::fread(out.data(), 1, out.size(), file);
	std::fclose(file);
	return read == out.size();
}

int main()
{
	TEST_SECTION("jpeg_save writes a decodable, correctly ordered screenshot");

	const int kWidth = 8;
	const int kHeight = 8;
	const char* kPath = "/tmp/openmt2_test_screenshot.jpg";

	// Left half red, right half blue, in the BGR order the screenshot path uses.
	std::vector<unsigned char> bgr((size_t) kWidth * kHeight * 3);
	for (int y = 0; y < kHeight; ++y)
	{
		for (int x = 0; x < kWidth; ++x)
		{
			unsigned char* px = &bgr[((size_t) y * kWidth + x) * 3];
			const bool left = x < kWidth / 2;
			px[0] = left ? 0 : 255;    // B
			px[1] = 0;                 // G
			px[2] = left ? 255 : 0;    // R
		}
	}

	std::remove(kPath);
	TEST_ASSERT(jpeg_save(bgr.data(), kWidth, kHeight, 85, kPath) != 0);

	std::vector<unsigned char> fileBytes;
	TEST_ASSERT(ReadFile(kPath, fileBytes));

	uint32_t width = 0;
	uint32_t height = 0;
	std::vector<uint8_t> bgra;
	TEST_ASSERT(Platform_DecodeImage(fileBytes.data(), fileBytes.size(), &width, &height, bgra));
	TEST_ASSERT(width == (uint32_t) kWidth);
	TEST_ASSERT(height == (uint32_t) kHeight);
	TEST_ASSERT(bgra.size() == (size_t) kWidth * kHeight * 4);

	// JPEG is lossy: allow a tolerance, but the red/blue order must be right.
	const unsigned char* left = &bgra[((size_t) (kHeight / 2) * kWidth + 1) * 4];
	const unsigned char* right = &bgra[((size_t) (kHeight / 2) * kWidth + kWidth - 2) * 4];

	TEST_ASSERT(left[0] < 60 && left[1] < 60 && left[2] > 190);   // BGRA red
	TEST_ASSERT(right[0] > 190 && right[1] < 60 && right[2] < 60); // BGRA blue

	std::remove(kPath);
	return 0;
}
