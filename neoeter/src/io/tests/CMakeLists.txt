# src/neoeter_io/tests/CMakeLists.txt

add_executable(test_eterpack test_eterpack.cpp)
target_link_libraries(test_eterpack PRIVATE neoeter_io)
add_test(NAME test_eterpack COMMAND test_eterpack)

add_executable(test_pack_formats test_pack_formats.cpp)
target_link_libraries(test_pack_formats PRIVATE neoeter_io)
add_test(NAME test_pack_formats COMMAND test_pack_formats)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_eterpack test_pack_formats)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()

# TA.3: libFuzzer target, built only by scripts/fuzz-smoke.sh.
if(OPENMT2_BUILD_FUZZERS)
    if(NOT CMAKE_CXX_COMPILER_ID MATCHES "Clang")
        message(FATAL_ERROR "Fuzzers require clang (-fsanitize=fuzzer)")
    endif()
    add_executable(fuzz_pack fuzz_pack.cpp)
    target_link_libraries(fuzz_pack PRIVATE neoeter_io)
    target_compile_options(fuzz_pack PRIVATE -fsanitize=fuzzer-no-link,address,undefined -UNDEBUG)
    target_link_options(fuzz_pack PRIVATE -fsanitize=fuzzer,address,undefined)
endif()
