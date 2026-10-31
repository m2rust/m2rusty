add_executable(test_mileslib
    test_mileslib.cpp
)

target_link_libraries(test_mileslib
    PRIVATE
        neoeter_audio
)

add_test(NAME MilesLib_SmokeTests COMMAND test_mileslib)

# TA.5: headless audio. The process must initialise against ma_backend_null and
# work without a sound card.
add_executable(test_miles_headless
    test_miles_headless.cpp
)

target_link_libraries(test_miles_headless
    PRIVATE
        neoeter_audio
)

add_test(NAME test_miles_headless COMMAND test_miles_headless)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_mileslib test_miles_headless)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()

# TA.5: every neoeter_audio test is headless by contract.
set_tests_properties(MilesLib_SmokeTests test_miles_headless PROPERTIES
    ENVIRONMENT "OPENMT2_AUDIO_BACKEND=null"
)
