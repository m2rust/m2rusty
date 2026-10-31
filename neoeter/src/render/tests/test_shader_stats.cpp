// T1.8 / D13 — the shader no-op must be measurable, not silent.
//
// The GL compatibility renderer creates dummy shader objects and discards
// binds and constant uploads. CShaderStats counts what a frame asks for so
// T5.2 can prove the delta closes.

#include <support/TestAssert.h>

#include <Neoeter/Render/ShaderStats.h>

#include <cstdio>
#include <string>
#include <vector>

namespace
{

void test_created_shaders()
{
	TEST_SECTION("created shaders");

	CShaderStats::Reset();
	CShaderStats::SCounters counters = CShaderStats::Get();
	TEST_ASSERT(counters.vertexCreated == 0);
	TEST_ASSERT(counters.pixelCreated == 0);
	TEST_ASSERT(counters.ignoredFiles == 0);

	CShaderStats::OnCreated(true, "effect/tree_branch.fx");
	CShaderStats::OnCreated(true, "effect/tree_branch.fx");
	CShaderStats::OnCreated(false, "effect/tree_leaf.fx");

	counters = CShaderStats::Get();
	TEST_ASSERT(counters.vertexCreated == 2);
	TEST_ASSERT(counters.pixelCreated == 1);
	TEST_ASSERT(counters.ignoredFiles == 2);

	// Creation without a file name (in-memory/compiled bytecode) is counted
	// under a placeholder instead of crashing or being dropped.
	CShaderStats::OnCreated(true, NULL);
	TEST_ASSERT(CShaderStats::Get().vertexCreated == 3);
	TEST_ASSERT(CShaderStats::Get().ignoredFiles == 3);

	const std::vector<std::string> files = CShaderStats::GetIgnoredFiles();
	TEST_ASSERT(files.size() == 3);
	TEST_ASSERT(files[0] == "<memory>");
	TEST_ASSERT(files[1] == "effect/tree_branch.fx");
	TEST_ASSERT(files[2] == "effect/tree_leaf.fx");

	TEST_PASS("creation is counted once per call, once per file");
}

void test_binds_and_constants()
{
	TEST_SECTION("binds and constant uploads");

	CShaderStats::Reset();

	for (int i = 0; i < 3; ++i)
		CShaderStats::OnBound(true);
	for (int i = 0; i < 2; ++i)
		CShaderStats::OnBound(false);
	for (int i = 0; i < 4; ++i)
		CShaderStats::OnConstants(true);
	CShaderStats::OnConstants(false);

	const CShaderStats::SCounters counters = CShaderStats::Get();
	TEST_ASSERT(counters.vertexBound == 3);
	TEST_ASSERT(counters.pixelBound == 2);
	TEST_ASSERT(counters.vertexConstantUploads == 4);
	TEST_ASSERT(counters.pixelConstantUploads == 1);
	TEST_ASSERT(counters.vertexCreated == 0);

	TEST_PASS("binds and uploads are counted per call");
}

void test_reset()
{
	TEST_SECTION("reset");

	CShaderStats::OnCreated(true, "a.fx");
	CShaderStats::OnBound(false);
	CShaderStats::OnConstants(true);
	CShaderStats::Reset();

	const CShaderStats::SCounters counters = CShaderStats::Get();
	TEST_ASSERT(counters.vertexCreated == 0);
	TEST_ASSERT(counters.pixelCreated == 0);
	TEST_ASSERT(counters.vertexBound == 0);
	TEST_ASSERT(counters.pixelBound == 0);
	TEST_ASSERT(counters.vertexConstantUploads == 0);
	TEST_ASSERT(counters.pixelConstantUploads == 0);
	TEST_ASSERT(counters.ignoredFiles == 0);
	TEST_ASSERT(CShaderStats::GetIgnoredFiles().empty());

	TEST_PASS("reset clears counters and the file set");
}

} // namespace

int main()
{
	test_created_shaders();
	test_binds_and_constants();
	test_reset();

	std::printf("Shader no-op statistics tests passed.\n");
	return 0;
}
