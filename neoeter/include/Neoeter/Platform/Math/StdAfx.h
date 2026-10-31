#pragma once

// T4.1: the DirectX SDK is gone; the platform headers carry the D3D9 vocabulary
// on every target (USE_PLATFORM_D3D_COMPAT on Windows).
#include <Platform/Platform.h>
#include <Platform/DirectXMathCompat.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <math.h>
