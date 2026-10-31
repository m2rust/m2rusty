add_executable(test_prterrainlib test_prterrainlib.cpp)

target_link_libraries(test_prterrainlib
    PRIVATE
        neoeter_terrain
)

add_test(NAME PRTerrainLib_SmokeTests COMMAND test_prterrainlib)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_prterrainlib)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
