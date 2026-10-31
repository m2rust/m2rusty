#pragma once

#include <cstddef>
#include <string>
#include <vector>

// D13: the GL compatibility renderer ignores shaders. `CreateVertexShader` /
// `CreatePixelShader` return dummy objects, `SetVertexShader*` /
// `SetPixelShader*` discard their arguments and `D3DXAssembleShaderFromFile*`
// returns an empty buffer, so every shader-driven visual effect silently
// falls back to fixed-function.
//
// These counters make the delta measurable instead of silent: the feature
// matrix (T1.8) records the static call sites, and this class records what a
// real frame actually creates/binds/uploads. T5.2 uses both.
class CShaderStats
{
	public:
		struct SCounters
		{
			unsigned int	vertexCreated = 0;
			unsigned int	pixelCreated = 0;
			unsigned int	vertexBound = 0;
			unsigned int	pixelBound = 0;
			unsigned int	vertexConstantUploads = 0;
			unsigned int	pixelConstantUploads = 0;
			std::size_t		ignoredFiles = 0;
		};

		static void		Reset();

		// Logs once per shader file, then counts every creation.
		static void		OnCreated(bool isVertex, const char* fileName);
		static void		OnBound(bool isVertex);
		static void		OnConstants(bool isVertex);

		static SCounters				Get();
		static std::vector<std::string>	GetIgnoredFiles();
};
