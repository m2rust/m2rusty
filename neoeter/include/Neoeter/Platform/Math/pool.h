/* Copyright (C) John W. Ratcliff, 2001.
 * All rights reserved worldwide.
 *
 * This software is provided "as is" without express or implied
 * warranties. You may freely copy and compile this source into
 * applications you distribute provided that the copyright text
 * below is included in the resulting source code, for example:
 * "Portions Copyright (C) John W. Ratcliff, 2001"
 */
#pragma once

/***********************************************************************/
/** POOL        : Template class to manage a fixed pool of items for   */
/**               extremely fast allocation and deallocation.          */
/**                                                                    */
/**               Written by John W. Ratcliff jratcliff@att.net        */
/***********************************************************************/

template <class Type> class Pool
{
  public:
    Pool()
        : mHead(nullptr), mCurrent(nullptr), mData(nullptr), mFree(nullptr), mFreeCount(0), mUsedCount(0) {

          };

    ~Pool()
    {

        delete[] mData;
    };

    void Release()
    {

        delete[] mData;

        mHead = 0;
        mFree = 0;

        mData = 0;
        mCurrent = 0;
        mFreeCount = 0;
        mUsedCount = 0;
    };

    void Set(int maxitems)
    {

        delete[] mData; // delete any previous incarnation.
        mMaxItems = maxitems;
        mData = new Type[mMaxItems];
        mFree = mData;
        mHead = nullptr;
        int const loopValue = (mMaxItems - 1);
        for (int i = 0; i < loopValue; i++)
        {
            mData[i].SetNext(&mData[i + 1]);
            if (i == 0)
            {
                mData[i].SetPrevious(nullptr);
            }
            else
            {
                mData[i].SetPrevious(&mData[i - 1]);
            }
        }

        mData[loopValue].SetNext(nullptr);
        mData[loopValue].SetPrevious(&mData[loopValue - 1]);
        mCurrent = nullptr; // there is no current, currently. <g>
        mFreeCount = maxitems;
        mUsedCount = 0;
    };

    Type *GetNext(bool &looped)
    {
        looped = false; // default value

        if (!mHead)
            return 0; // there is no data to process.
        Type *ret = nullptr;

        if (!mCurrent)
        {
            ret = mHead;
            looped = true;
        }
        else
        {
            ret = mCurrent;
        }

        if (ret)
            mCurrent = ret->GetNext();

        return ret;
    };

    bool IsEmpty() const
    {
        return mHead == nullptr;
    };

    int Begin()
    {
        mCurrent = mHead;
        return mUsedCount;
    };

    int GetUsedCount() const
    {
        return mUsedCount;
    };
    int GetFreeCount() const
    {
        return mFreeCount;
    };

    Type *GetNext()
    {
        if (!mHead)
            return 0; // there is no data to process.

        Type *ret = nullptr;

        if (!mCurrent)
        {
            ret = mHead;
        }
        else
        {
            ret = mCurrent;
        }

        if (ret)
            mCurrent = ret->GetNext();

        return ret;
    };

    void Release(Type *t)
    {
        if (t == mCurrent)
            mCurrent = t->GetNext();

        // first patch old linked list.. his previous now points to his next
        Type *prev = t->GetPrevious();

        if (prev)
        {
            Type *next = t->GetNext();
            prev->SetNext(next); // my previous now points to my next
            if (next)
                next->SetPrevious(prev);
            // list is patched!
        }
        else
        {
            Type *next = t->GetNext();
            mHead = next;
            if (mHead)
                mHead->SetPrevious(nullptr);
        }

        Type *temp = mFree; // old head of free list.
        mFree = t;          // new head of linked list.
        t->SetPrevious(nullptr);
        t->SetNext(temp);

        mUsedCount--;
        mFreeCount++;
    };

    Type *GetFreeNoLink() // get free, but don't link it to the used list!!
    {
        // Free allocated items are always added to the head of the list
        if (!mFree)
            return 0;
        Type *ret = mFree;
        mFree = ret->GetNext(); // new head of free list
        mUsedCount++;
        mFreeCount--;
        ret->SetNext(0);
        ret->SetPrevious(0);
        return ret;
    };

    Type *GetFreeLink()
    {
        // Free allocated items are always added to the head of the list
        if (!mFree)
            return nullptr;
        Type *ret = mFree;
        mFree = ret->GetNext(); // new head of free list
        Type *temp = mHead;     // current head of list
        mHead = ret;            // new head of list is this free one
        if (temp)
            temp->SetPrevious(ret);
        mHead->SetNext(temp);
        mHead->SetPrevious(nullptr);
        mUsedCount++;
        mFreeCount--;
        return ret;
    };

    void AddAfter(Type *e, Type *item)
    {
        // Add 'item' after 'e'
        if (e)
        {
            Type *eprev = e->GetPrevious();
            Type *enext = e->GetNext();
            e->SetNext(item);
            item->SetNext(enext);
            item->SetPrevious(e);
            if (enext)
                enext->SetPrevious(item);
        }
        else
        {
            mHead = item;
            item->SetPrevious(0);
            item->SetNext(0);
        }
    }

    void AddBefore(Type *e, Type *item)
    {
        // Add 'item' before 'e'
        Type *eprev = e->GetPrevious();
        Type *enext = e->GetNext();

        if (!eprev)
        {
            mHead = item;
        }
        else
        {
            eprev->SetNext(item);
        }

        item->SetPrevious(eprev);
        item->SetNext(e);

        e->SetPrevious(item);
    }

  private:
    int mMaxItems = 0;
    Type *mCurrent; // current iteration location.
    Type *mData;
    Type *mHead; // head of used list.
    Type *mFree; // head of free list.
    int mUsedCount;
    int mFreeCount;
};
