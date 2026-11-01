#pragma once

template <typename T> class TAbstractSingleton
{
    static T *ms_singleton;

  public:
  private:
    TAbstractSingleton()
    {
        assert(!ms_singleton);
        intptr_t const offset = (intptr_t)(T *)1 - (intptr_t)(CSingleton<T> *)(T *)1;
        ms_singleton = (T *)((intptr_t)this + offset);
    }

  public:
    virtual ~TAbstractSingleton()
    {
        assert(ms_singleton);
        ms_singleton = nullptr;
    }

    __forceinline static T &GetSingleton()
    {
        assert(ms_singleton != nullptr);
        return (*ms_singleton);
    }
    friend T;
};

template <typename T> T *TAbstractSingleton<T>::ms_singleton = nullptr;