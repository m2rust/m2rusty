#pragma once

class CPropertyManager : public CSingleton<CPropertyManager>
{
	public:
		CPropertyManager();
		virtual ~CPropertyManager();

		void			Clear();

		bool			LoadReservedCRC(const char * c_pszFileName);
		void			ReserveCRC(DWORD dwCRC);
		DWORD			GetUniqueCRC(const char * c_szSeed);

		void			Initialize();
		bool			Register(const char * c_pszFileName, CProperty ** ppProperty = NULL);

		bool			Get(DWORD dwCRC, CProperty ** ppProperty);
		bool			Get(const char * c_pszFileName, CProperty ** ppProperty);

	protected:
		typedef std::map<DWORD, CProperty *>		TPropertyCRCMap;
		typedef std::set<DWORD>						TCRCSet;

		TPropertyCRCMap								m_PropertyByCRCMap;
		TCRCSet										m_ReservedCRCSet;
};
