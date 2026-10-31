#pragma once

#define WIN32_LEAN_AND_MEAN	
#ifndef _CRT_SECURE_NO_WARNINGS
#define _CRT_SECURE_NO_WARNINGS
#endif

#define _WIN32_DCOM

#ifdef _MSC_VER
#pragma warning(disable:4710)	// not inlined
#pragma warning(disable:4786)	// character 255 넘어가는거 끄기
#pragma warning(disable:4244)	// type conversion possible lose of data

#pragma warning(disable:4018)
#pragma warning(disable:4245)
#pragma warning(disable:4512)
#pragma warning(disable:4201)

#if _MSC_VER >= 1400
#pragma warning(disable:4201 4512 4238 4239)
#endif
#endif

#ifdef _WIN32
// T4.1: the DirectX SDK is gone. D3D9 (and the D3DX helpers the client uses)
// come from the compatibility header, which the Windows build selects with
// USE_PLATFORM_D3D_COMPAT; the platform libraries are linked by CMake.
#include "Platform/DirectXMathCompat.h"

#pragma warning ( disable : 4201 )
#include <mmsystem.h>
#pragma warning ( default : 4201 )
#include <process.h>
#include <direct.h>
#include <malloc.h>
#include <io.h>

#ifndef VC_EXTRALEAN
#include <winsock.h>
#endif
#else
#include "Platform/Platform.h"
#include "Platform/DirectXMathCompat.h"
#endif

#include <stdio.h>
#include <math.h>
#include <time.h>

#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/Debug.h>
#include <Neoeter/Platform/Locale/CodePageId.h>


/*
#include "Pool.h"
#include "Dynamic.h"
#include "Event.h"
#include "FuncObject.h"
#include "ReferenceObject.h"
#include "Ref.h"

#include "Util.h"
#include "TextFileLoader.h"
#include "Parser.h"

#include "Resource.h"
#include "ResourceManager.h"

#include "MSWindow.h"
#include "MSApplication.h"
#include "Mutex.h"
#include "Thread.h"

#include "GrpBase.h"

#include "GrpDIB.h"
#include "GrpMath.h"
#include "GrpDevice.h"

#include "CollisionData.h"
#include "GrpCollisionObject.h"
#include "GrpScreen.h"
#include "CullingManager.h"

// Attribute
#include "AttributeData.h"
#include "AttributeInstance.h"

#include "GrpObjectInstance.h"
#include "GrpRatioInstance.h"

#include "GrpD3DXBuffer.h"

#include "GrpTexture.h"
#include "GrpImageTexture.h"
#include "GrpFontTexture.h"

#include "GrpText.h"
#include "GrpImage.h"
#include "GrpSubImage.h"

#include "GrpIndexBuffer.h"
#include "GrpVertexBuffer.h"
#include "GrpVertexBufferStatic.h"
#include "GrpVertexBufferDynamic.h"
#include "GrpVertexShader.h"
#include "GrpPixelShader.h"


#include "GrpShadowTexture.h"
#include "GrpImageInstance.h"
#include "GrpExpandedImageInstance.h"
#include "GrpTextInstance.h"
#include "GrpLightManager.h"

#include "TargaResource.h"

#include "NetDevice.h"
#include "NetAddress.h"
// #include "NetStream.h"
#include "NetPacketHeaderMap.h"
#include "NetDatagramSender.h"
#include "NetDatagramReceiver.h"

#include "Input.h"
#include "IME.h"

#include "PathStack.h"
//#include "Property.h"

#include "Profiler.h"

#include "StateManager.h"

#include "ColorTransitionHelper.h"
#include "LensFlare.h"
#include "ScreenFilter.h"
#include "EnvironmentMap.h"

#include "lineintersect_utils.h"

#include "Decal.h"
*/
