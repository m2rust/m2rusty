#pragma once

class CPythonSystem : public CSingleton<CPythonSystem>
{
  public:
    enum EWindow
    {
        WINDOW_STATUS,
        WINDOW_INVENTORY,
        WINDOW_ABILITY,
        WINDOW_SOCIETY,
        WINDOW_JOURNAL,
        WINDOW_COMMAND,

        WINDOW_QUICK,
        WINDOW_GAUGE,
        WINDOW_MINIMAP,
        WINDOW_CHAT,

        WINDOW_MAX_NUM,
    };

    enum
    {
        FREQUENCY_MAX_NUM = 30,
        RESOLUTION_MAX_NUM = 100
    };

    using TResolution = struct SResolution
    {
        DWORD width;
        DWORD height;
        DWORD bpp; // bits per pixel (high-color = 16bpp, true-color = 32bpp)

        DWORD frequency[20];
        BYTE frequency_count;
    };

    using TWindowStatus = struct SWindowStatus
    {
        int isVisible;
        int isMinimized;

        int ixPosition;
        int iyPosition;
        int iHeight;
    };

    using TConfig = struct SConfig
    {
        DWORD width;
        DWORD height;
        DWORD bpp;
        DWORD frequency;

        bool is_software_cursor;
        bool is_object_culling;
        int iDistance;
        int iShadowLevel;

        FLOAT music_volume;
        BYTE voice_volume;

        int gamma;

        int isSaveID;
        char SaveID[20];

        bool bWindowed;
        bool bDecompressDDS;
        bool bNoSoundCard;
        bool bUseDefaultIME;
        BYTE bSoftwareTiling;
        bool bViewChat;
        bool bAlwaysShowName;
        bool bShowDamage;
        bool bShowSalesText;
    };

    CPythonSystem();
    ~CPythonSystem() override;

    void Clear();
    void SetInterfaceHandler(PyObject *poHandler);
    void DestroyInterfaceHandler();

    // Config
    void SetDefaultConfig();
    bool LoadConfig();
    bool SaveConfig();
    void ApplyConfig();
    void SetConfig(TConfig *pNewConfig);
    TConfig *GetConfig();
    void ChangeSystem() const;

    // Interface
    bool LoadInterfaceStatus();
    void SaveInterfaceStatus();
    bool isInterfaceConfig() const;
    const TWindowStatus &GetWindowStatusReference(int iIndex);

    DWORD GetWidth() const;
    DWORD GetHeight() const;
    DWORD GetBPP() const;
    DWORD GetFrequency() const;
    bool IsSoftwareCursor() const;
    bool IsWindowed() const;
    bool IsViewChat() const;
    bool IsAlwaysShowName() const;
    bool IsShowDamage() const;
    bool IsShowSalesText() const;
    bool IsUseDefaultIME() const;
    bool IsNoSoundCard() const;
    bool IsAutoTiling() const;
    bool IsSoftwareTiling() const;
    void SetSoftwareTiling(bool isEnable);
    void SetViewChatFlag(int iFlag);
    void SetAlwaysShowNameFlag(int iFlag);
    void SetShowDamageFlag(int iFlag);
    void SetShowSalesTextFlag(int iFlag);

    // Window
    void SaveWindowStatus(int iIndex, int iVisible, int iMinimized, int ix, int iy, int iHeight);

    // SaveID
    int IsSaveID() const;
    const char *GetSaveID();
    void SetSaveID(int iValue, const char *c_szSaveID);

    /// Display
    void GetDisplaySettings();

    int GetResolutionCount() const;
    int GetFrequencyCount(int index);
    bool GetResolution(int index, OUT DWORD *width, OUT DWORD *height, OUT DWORD *bpp);
    bool GetFrequency(int index, int freq_index, OUT DWORD *frequncy);
    int GetResolutionIndex(DWORD width, DWORD height, DWORD bit);
    int GetFrequencyIndex(int res_index, DWORD frequency);
    bool isViewCulling() const;

    // Sound
    float GetMusicVolume() const;
    int GetSoundVolume() const;
    void SetMusicVolume(float fVolume);
    void SetSoundVolumef(float fVolume);

    int GetDistance() const;
    int GetShadowLevel() const;
    void SetShadowLevel(unsigned int level);

  protected:
    TResolution m_ResolutionList[RESOLUTION_MAX_NUM]{};
    int m_ResolutionCount = 0;

    TConfig m_Config{};
    TConfig m_OldConfig{};

    bool m_isInterfaceConfig;
    PyObject *m_poInterfaceHandler;
    TWindowStatus m_WindowStatus[WINDOW_MAX_NUM]{};
};