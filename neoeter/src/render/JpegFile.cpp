#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/JpegFile.h>

#include <vector>

#include <Neoeter/Platform/Base/ImageLoaderCompat.h>

int jpeg_save(unsigned char* data, int width, int height, int quality, const char* filename)
{
	if (!data || width <= 0 || height <= 0 || !filename)
		return 0;

	// The screenshot path fills the buffer as BGR; stb_image_write expects RGB.
	const size_t count = (size_t) width * (size_t) height;
	std::vector<unsigned char> rgb(count * 3);

	for (size_t i = 0; i < count; ++i)
	{
		rgb[i * 3 + 0] = data[i * 3 + 2];
		rgb[i * 3 + 1] = data[i * 3 + 1];
		rgb[i * 3 + 2] = data[i * 3 + 0];
	}

	return Platform_SaveImageJPEG(filename, width, height, rgb.data(), quality) ? 1 : 0;
}
