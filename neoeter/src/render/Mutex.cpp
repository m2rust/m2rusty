#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/Mutex.h>

Mutex::Mutex()
{
	InitializeCriticalSection(&lock);
}

Mutex::~Mutex()
{
	DeleteCriticalSection(&lock);
}

void Mutex::Lock()
{
	EnterCriticalSection(&lock);
}

void Mutex::Unlock()
{
	LeaveCriticalSection(&lock);
}