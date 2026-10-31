#include <Neoeter/Python/Host/StdAfx.h>
#include <Neoeter/Render/GrpExpandedImageInstance.h>
#include <Neoeter/Render/GrpTextInstance.h>
#include <Neoeter/Render/GrpMarkInstance.h>
#include <Neoeter/Render/GrpSubImage.h>
#include <Neoeter/Render/GrpText.h>
#include <Neoeter/Render/AttributeData.h>
#include <Neoeter/Model/Grn/Thing.h>
#include <Neoeter/Model/Grn/ThingInstance.h>
#include <Neoeter/Effect/EffectMesh.h>
#include <Neoeter/Effect/EffectInstance.h>

#include <Neoeter/Python/Host/Resource.h>

std::vector<CPythonResource::TDestroyHandler> CPythonResource::ms_destroyHandlers;

CResource * NewImage(const char* c_szFileName)
{
	return new CGraphicImage(c_szFileName);
}

CResource * NewSubImage(const char* c_szFileName)
{
	return new CGraphicSubImage(c_szFileName);
}

CResource * NewText(const char* c_szFileName)
{
	return new CGraphicText(c_szFileName);
}

CResource * NewThing(const char* c_szFileName)
{
	return new CGraphicThing(c_szFileName);
}

CResource * NewEffectMesh(const char* c_szFileName)
{
	return new CEffectMesh(c_szFileName);
}

CResource * NewAttributeData(const char* c_szFileName)
{
	return new CAttributeData(c_szFileName);
}

void CPythonResource::RegisterDestroyHandler(TDestroyHandler pfnHandler)
{
	if (pfnHandler)
		ms_destroyHandlers.push_back(pfnHandler);
}

void CPythonResource::DumpFileList(const char * c_szFileName)
{
	m_resManager.DumpFileListToTextFile(c_szFileName);
}

void CPythonResource::Destroy()
{
	for (size_t i = 0; i < ms_destroyHandlers.size(); ++i)
	{
		if (ms_destroyHandlers[i])
			ms_destroyHandlers[i]();
	}

	CGraphicExpandedImageInstance::DestroySystem();
	CGraphicImageInstance::DestroySystem();	
	CGraphicMarkInstance::DestroySystem();
	CGraphicThingInstance::DestroySystem();
	CGrannyModelInstance::DestroySystem();
	CGraphicTextInstance::DestroySystem();
	CEffectInstance::DestroySystem();
	
	m_resManager.DestroyDeletingList();

	CEffectData::DestroySystem();
	CEffectMesh::SEffectMeshData::DestroySystem();

	m_resManager.Destroy();	
}

CPythonResource::CPythonResource()
{
	m_resManager.RegisterResourceNewFunctionPointer("sub", NewSubImage);
	m_resManager.RegisterResourceNewFunctionPointer("dds", NewImage);
	m_resManager.RegisterResourceNewFunctionPointer("jpg", NewImage);
	m_resManager.RegisterResourceNewFunctionPointer("tga", NewImage);
	m_resManager.RegisterResourceNewFunctionPointer("bmp", NewImage);
	m_resManager.RegisterResourceNewFunctionPointer("fnt", NewText);
	m_resManager.RegisterResourceNewFunctionPointer("gr2", NewThing);
	m_resManager.RegisterResourceNewFunctionPointer("mde", NewEffectMesh);
	m_resManager.RegisterResourceNewFunctionPointer("mdatr", NewAttributeData);
}

CPythonResource::~CPythonResource()
{
	Destroy();
}
