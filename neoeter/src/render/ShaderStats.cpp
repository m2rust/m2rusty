#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/ShaderStats.h>

#include <Neoeter/Platform/Base/Debug.h>

#include <set>

namespace
{

struct SShaderStatsState
{
	CShaderStats::SCounters counters;
	std::set<std::string> files;
};

SShaderStatsState& GetState()
{
	static SShaderStatsState state;
	return state;
}

} // namespace

void CShaderStats::Reset()
{
	GetState() = SShaderStatsState();
}

void CShaderStats::OnCreated(bool isVertex, const char* fileName)
{
	SShaderStatsState& state = GetState();
	if (isVertex)
		++state.counters.vertexCreated;
	else
		++state.counters.pixelCreated;

	const std::string name = (fileName && fileName[0]) ? fileName : "<memory>";
	if (state.files.insert(name).second)
	{
		// One line per shader file, not per instance: shaders are created once
		// and then reused, and a flood of identical lines hides the signal.
		TraceError("[SHADER] '%s' is ignored by the GL renderer (D13, T5.2)",
				   name.c_str());
	}
}

void CShaderStats::OnBound(bool isVertex)
{
	SShaderStatsState& state = GetState();
	if (isVertex)
		++state.counters.vertexBound;
	else
		++state.counters.pixelBound;
}

void CShaderStats::OnConstants(bool isVertex)
{
	SShaderStatsState& state = GetState();
	if (isVertex)
		++state.counters.vertexConstantUploads;
	else
		++state.counters.pixelConstantUploads;
}

CShaderStats::SCounters CShaderStats::Get()
{
	SShaderStatsState& state = GetState();
	SCounters counters = state.counters;
	counters.ignoredFiles = state.files.size();
	return counters;
}

std::vector<std::string> CShaderStats::GetIgnoredFiles()
{
	SShaderStatsState& state = GetState();
	return std::vector<std::string>(state.files.begin(), state.files.end());
}
