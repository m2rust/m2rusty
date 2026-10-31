add_executable(test_effectlib test_effectlib.cpp)

target_link_libraries(test_effectlib
    PRIVATE
        neoeter_effect
)

add_test(NAME EffectLib_SmokeTests COMMAND test_effectlib)

# TA.5: fixed-step particle updates and seeded emission are reproducible.
add_executable(test_effect_determinism test_effect_determinism.cpp)

target_link_libraries(test_effect_determinism
    PRIVATE
        neoeter_effect
)

add_test(NAME test_effect_determinism COMMAND test_effect_determinism)

# TA.1: shared test helpers and always-on asserts (tests/support/TestAssert.h)
foreach(_test_target test_effectlib test_effect_determinism)
    target_include_directories(${_test_target} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_compile_options(${_test_target} PRIVATE $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-UNDEBUG>)
endforeach()
