add_executable(test_etergrnlib test_etergrnlib.cpp)

target_link_libraries(test_etergrnlib
    PRIVATE
        neoeter_model_grn
)

add_test(NAME EterGrnLib_SmokeTests COMMAND test_etergrnlib)

add_executable(test_gr2_reader test_gr2_reader.cpp)
target_link_libraries(test_gr2_reader PRIVATE neoeter_model_grn)
add_test(NAME test_gr2_reader COMMAND test_gr2_reader)

# TA.5: CPU-only reference values for world poses, skinning and curves.
add_executable(test_grn_reference test_grn_reference.cpp)
target_link_libraries(test_grn_reference PRIVATE neoeter_model_grn)
add_test(NAME test_grn_reference COMMAND test_grn_reference)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_etergrnlib test_gr2_reader test_grn_reference)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()

# TA.3: libFuzzer target, built only by scripts/fuzz-smoke.sh.
if(OPENMT2_BUILD_FUZZERS)
    if(NOT CMAKE_CXX_COMPILER_ID MATCHES "Clang")
        message(FATAL_ERROR "Fuzzers require clang (-fsanitize=fuzzer)")
    endif()
    add_executable(fuzz_gr2 fuzz_gr2.cpp)
    target_link_libraries(fuzz_gr2 PRIVATE neoeter_model_grn)
    target_compile_options(fuzz_gr2 PRIVATE -fsanitize=fuzzer-no-link,address,undefined -UNDEBUG)
    target_link_options(fuzz_gr2 PRIVATE -fsanitize=fuzzer,address,undefined)
endif()
