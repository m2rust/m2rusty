add_executable(m2diff_effect EffectDump.cpp)
target_link_libraries(m2diff_effect PRIVATE client_game neoeter_effect neoeter_render neoeter_io)
# Every texture request of the loaders goes through
# CResourceManager::GetResourcePointer; the dumper records the names instead
# of creating images (see __wrap_ in EffectDump.cpp).
target_link_options(m2diff_effect PRIVATE "LINKER:--wrap=_ZN16CResourceManager18GetResourcePointerEPKc")
