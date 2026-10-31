#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/FontLocator.h>

#if defined(OPENMT2_PLATFORM_LINUX)
#include <cstdlib>
#include <map>
#include <mutex>
#include <unistd.h>

#if defined(OPENMT2_HAVE_FONTCONFIG)
#include <fontconfig/fontconfig.h>

static std::string MatchFontconfig(const char* family)
{
	static bool s_inited = FcInit() == FcTrue;
	if (!s_inited)
		return std::string();

	FcPattern* pattern = FcNameParse((const FcChar8*)family);
	if (!pattern)
		return std::string();

	FcPatternAddString(pattern, FC_FONTFORMAT, (const FcChar8*)"TrueType");
	FcConfigSubstitute(nullptr, pattern, FcMatchPattern);
	FcDefaultSubstitute(pattern);

	std::string result;
	FcResult res;
	FcPattern* match = FcFontMatch(nullptr, pattern, &res);
	if (match)
	{
		FcChar8* file = nullptr;
		if (FcPatternGetString(match, FC_FILE, 0, &file) == FcResultMatch && file)
			result = (const char*)file;
		FcPatternDestroy(match);
	}
	FcPatternDestroy(pattern);
	return result;
}
#endif

static bool IsReadable(const std::string& path)
{
	return !path.empty() && access(path.c_str(), R_OK) == 0;
}

static bool IsPlainAscii(const char* s)
{
	for (; *s; ++s)
		if ((unsigned char)*s >= 0x80)
			return false;
	return true;
}

static std::string Resolve(const char* fontName)
{
	if (fontName && fontName[0] && IsReadable(fontName))
		return fontName;

	if (const char* env = getenv("OPENMT2_FONT"))
		if (IsReadable(env))
			return env;

#if defined(OPENMT2_HAVE_FONTCONFIG)
	// Locale data can carry non-ASCII (e.g. Korean) face names; skip those.
	if (fontName && fontName[0] && IsPlainAscii(fontName))
	{
		std::string path = MatchFontconfig(fontName);
		if (IsReadable(path))
			return path;
	}

	std::string path = MatchFontconfig("sans-serif");
	if (IsReadable(path))
		return path;
#endif

	static const char* s_fontPaths[] = {
		"/usr/share/fonts/TTF/DejaVuSans.ttf",
		"/usr/share/fonts/noto/NotoSans-Regular.ttf",
		"/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
		"/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
		"/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
		"/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
		"/usr/share/fonts/truetype/freefont/FreeSans.ttf",
		nullptr
	};
	for (int i = 0; s_fontPaths[i]; ++i)
		if (IsReadable(s_fontPaths[i]))
			return s_fontPaths[i];

	return std::string();
}

std::string FontLocator_FindFontFile(const char* fontName)
{
	static std::mutex s_lock;
	static std::map<std::string, std::string> s_cache;

	std::string key = fontName ? fontName : "";
	std::lock_guard<std::mutex> guard(s_lock);

	auto it = s_cache.find(key);
	if (it != s_cache.end())
		return it->second;

	std::string path = Resolve(fontName);
	if (path.empty())
		TraceError("No font file found for '%s'; set OPENMT2_FONT to a .ttf path", key.c_str());
	else
		Tracef("Font '%s' -> %s\n", key.c_str(), path.c_str());

	s_cache[key] = path;
	return path;
}
#endif
