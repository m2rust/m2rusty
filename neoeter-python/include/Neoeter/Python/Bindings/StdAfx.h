#pragma once

#ifndef _CRT_SECURE_NO_WARNINGS
#define _CRT_SECURE_NO_WARNINGS
#endif

#include <Platform/Platform.h>
#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Python/Host/StdAfx.h>

#include <Neoeter/Python/Bindings/PythonGraphic.h>
#include <Neoeter/Python/Bindings/PythonWindowManager.h>

void initgrp();
void initgrpImage();
void initgrpText();
void initgrpThing();
void initscriptWindow();
void initwndMgr();
