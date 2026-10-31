#include <Client/UI/StdAfx.h>
#include <Client/UI/ImageFile.h>

#include <cstdio>
#include <vector>

#include <Neoeter/Platform/Base/ImageLoaderCompat.h>

static bool ReadFileBytes(const char* path, std::vector<uint8_t>& out)
{
	out.clear();

	FILE* file = fopen(path, "rb");
	if (!file)
		return false;

	fseek(file, 0, SEEK_END);
	long size = ftell(file);
	fseek(file, 0, SEEK_SET);

	if (size <= 0)
	{
		fclose(file);
		return false;
	}

	out.resize((size_t)size);
	size_t read = fread(out.data(), 1, out.size(), file);
	fclose(file);

	if (read != out.size())
	{
		out.clear();
		return false;
	}

	return true;
}

bool GetImageFileInfo(const char* path, uint32_t* width, uint32_t* height, uint32_t* format)
{
	std::vector<uint8_t> bytes;
	if (!path || !ReadFileBytes(path, bytes))
		return false;

	return Platform_GetImageInfo(bytes.data(), bytes.size(), width, height, format);
}

bool LoadImageFileBGRA(const char* path, uint32_t* width, uint32_t* height, std::vector<uint8_t>& pixels)
{
	std::vector<uint8_t> bytes;
	if (!path || !ReadFileBytes(path, bytes))
		return false;

	return Platform_DecodeImage(bytes.data(), bytes.size(), width, height, pixels);
}
