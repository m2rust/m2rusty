add_executable(test_speedtreelib test_speedtreelib.cpp)

target_link_libraries(test_speedtreelib
    PRIVATE
        neoeter_model_speedtree
)

add_test(NAME SpeedTreeLib_SmokeTests COMMAND test_speedtreelib)

add_executable(test_treecache test_treecache.cpp)
target_link_libraries(test_treecache PRIVATE neoeter_model_speedtree)
add_test(NAME test_treecache COMMAND test_treecache)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_speedtreelib test_treecache)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()

# TA.3: libFuzzer target, built only by scripts/fuzz-smoke.sh.
if(OPENMT2_BUILD_FUZZERS)
    if(NOT CMAKE_CXX_COMPILER_ID MATCHES "Clang")
        message(FATAL_ERROR "Fuzzers require clang (-fsanitize=fuzzer)")
    endif()
    add_executable(fuzz_treecache fuzz_treecache.cpp)
    target_link_libraries(fuzz_treecache PRIVATE neoeter_model_speedtree)
    target_compile_options(fuzz_treecache PRIVATE -fsanitize=fuzzer-no-link,address,undefined -UNDEBUG)
    target_link_options(fuzz_treecache PRIVATE -fsanitize=fuzzer,address,undefined)
endif()
