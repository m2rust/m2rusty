add_executable(m2diff_effect_runtime EffectRuntimeDump.cpp)
target_link_libraries(m2diff_effect_runtime PRIVATE client_game neoeter_effect neoeter_render neoeter_io)
# Texture requests go through CResourceManager::GetResourcePointer; the dumper
# answers them itself (null for textures, the real CEffectMesh for .mde).
target_link_options(m2diff_effect_runtime PRIVATE
    "LINKER:--wrap=_ZN16CResourceManager18GetResourcePointerEPKc"
    # Billboard transforms set the texture factor through the state manager;
    # there is no device here.
    "LINKER:--wrap=_ZN13CStateManager14SetRenderStateE19_D3DRENDERSTATETYPEj")
