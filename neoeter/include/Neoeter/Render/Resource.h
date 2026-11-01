#pragma once

#include "ReferenceObject.h"
#include <string>

class CResource : public CReferenceObject
{
  public:
    using TType = DWORD;

    enum EState
    {
        STATE_EMPTY,
        STATE_ERROR,
        STATE_EXIST,
        STATE_LOAD,
        STATE_FREE
    };

    void Clear();

    static TType StringToType(const char *c_szType);
    static TType Type();

    void Load();
    void Reload();
    int ConvertPathName(const char *c_szPathName, char *pszRetPathName, int retLen);

    virtual bool CreateDeviceObjects();
    virtual void DestroyDeviceObjects();

    CResource(const char *c_szFileName);
    ~CResource() override;

    static void SetDeleteImmediately(bool isSet = false);

    // is loaded?
    bool IsData() const;
    bool IsEmpty() const;
    bool IsType(TType type);

    DWORD GetLoadCostMilliSecond() const
    {
        return m_dwLoadCostMiliiSecond;
    }
    // const char *	GetFileName() const			{ return
    // m_pszFileName;				}
    const char *GetFileName() const
    {
        return m_stFileName.c_str();
    }
    const std::string &GetFileNameString() const
    {
        return m_stFileName;
    }

    virtual bool OnLoad(int iSize, const void *c_pvBuf) = 0;

  protected:
    void SetFileName(const char *c_szFileName);

    virtual void OnClear() = 0;
    virtual bool OnIsEmpty() const = 0;
    virtual bool OnIsType(TType type) = 0;

    void OnConstruct() override;
    void OnSelfDestruct() override;

    std::string m_stFileName;
    // char *			m_pszFileName;
    DWORD m_dwLoadCostMiliiSecond = 0U;
    EState me_state = STATE_EMPTY;

    static bool ms_bDeleteImmediately;
};
