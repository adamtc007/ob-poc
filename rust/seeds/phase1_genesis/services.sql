--
-- PostgreSQL database dump
--

\restrict FGaAzPiFXh7NgkxdzCPJRsyOgaGVsyeVbbZ4JVwFAR2746CirgIzZTxKLzyMfaG

-- Dumped from database version 18.1 (Homebrew)
-- Dumped by pg_dump version 18.1 (Homebrew)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: services; Type: TABLE DATA; Schema: ob-poc; Owner: adamtc007
--

COPY "ob-poc".services (service_id, name, description, created_at, updated_at, service_code, service_category, sla_definition, is_active, lifecycle_tags, lifecycle_status) FROM stdin;
631b59f4-7317-432c-a4b2-64c12c643cdd	Income Collection	Dividend and interest collection	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	INCOME_COLLECT	Operations	\N	t	{}	ungoverned
24c54be5-c507-45ce-9955-b52212d09b95	Proxy Voting	Proxy voting and shareholder services	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	PROXY_VOTING	Governance	\N	t	{}	ungoverned
449435fb-6b07-4c46-b0f5-77bee288b305	FX Execution	Foreign exchange execution services	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	FX_EXECUTION	Trading	\N	t	{}	ungoverned
e4140ad9-9d91-46c1-913d-ef38a06a12c3	Fund Reporting	Regulatory and investor reporting	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	FUND_REPORTING	Reporting	\N	t	{}	ungoverned
096f3dc8-3619-46ea-b2ba-a9a41c79bdcb	Expense Management	Fund expense accrual and payment	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	EXPENSE_MGMT	Accounting	\N	t	{}	ungoverned
6f3b44ff-bbbf-4f61-a368-3436aaac6acd	Performance Measurement	Performance calculation and attribution	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	PERF_MEASURE	Analytics	\N	t	{}	ungoverned
91f5e266-8802-4522-bbd2-8501831a5ccb	Position Management	Real-time position tracking	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	POSITION_MGMT	IBOR	\N	t	{}	ungoverned
555cea6c-d32b-4490-8705-9a157f48644d	Trade Capture	Trade booking and lifecycle management	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	TRADE_CAPTURE	IBOR	\N	t	{}	ungoverned
f4ad454a-4479-4bec-b922-a7f45922dc9d	Collateral Management	Collateral optimization and margin management	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	COLLATERAL_MGMT	Operations	\N	t	{}	ungoverned
d2d71198-dd5b-4f66-9547-ff76bf242e0a	Positions Reconciliation	\N	2025-12-03 16:25:44.363837+00	2025-12-03 16:25:44.363837+00	RECON_POSITIONS	Reconciliation	\N	t	{}	ungoverned
4a560fb4-b635-4f84-a4ad-fba24b08818a	Transactions Reconciliation	\N	2025-12-03 16:25:44.363837+00	2025-12-03 16:25:44.363837+00	RECON_TRANSACTIONS	Reconciliation	\N	t	{}	ungoverned
e64867f9-67e4-4288-96ae-76df27e52376	NAV Dissemination	\N	2025-12-03 16:28:07.8773+00	2025-12-03 16:28:07.8773+00	NAV_DISSEM	Valuation	\N	t	{}	ungoverned
ff28326c-c9ac-4752-b704-339a4263e17a	Asset Pricing	\N	2025-12-03 16:28:07.8773+00	2025-12-03 16:28:07.8773+00	ASSET_PRICING	Valuation	\N	t	{}	ungoverned
2f43e57d-1de8-462a-bf6c-7460f4dfa3a1	Variation Margining	\N	2025-12-03 16:28:07.8773+00	2025-12-03 16:28:07.8773+00	VAR_MARGIN	Collateral	\N	t	{}	ungoverned
3fcc8612-d9e0-4346-a2d4-a4f95451cf87	Withholding Tax	\N	2025-12-03 16:28:07.8773+00	2025-12-03 16:28:07.8773+00	WITHHOLD_TAX	Tax	\N	t	{}	ungoverned
045b5f42-b484-406f-a8f4-c44a3281e13f	MiFID Regulatory	\N	2025-12-03 16:28:07.8773+00	2025-12-03 16:28:07.8773+00	MIFID_REG	Regulatory	\N	t	{}	ungoverned
e559f68b-120f-4a4e-9c09-af5793e1c860	KYC as a Service	\N	2025-12-03 16:29:18.55705+00	2025-12-03 16:29:18.55705+00	KYC_SERVICE	Compliance	\N	t	{}	ungoverned
911ed95b-97a1-4dfd-95f8-08c038e8ebe5	ManCo Reporting	\N	2025-12-03 16:29:18.55705+00	2025-12-03 16:29:18.55705+00	MANCO_REPORTING	Reporting	\N	t	{}	ungoverned
c50cda37-ad92-4437-bf72-18d12302da1f	CapStock Automation	\N	2025-12-03 16:29:18.55705+00	2025-12-03 16:29:18.55705+00	CAPSTOCK_AUTO	Corporate Actions	\N	t	{}	ungoverned
511d2ddf-1509-4627-ad78-fdf94fb7115d	Hedge Fund Accounting	Accounting services for hedge funds	2025-12-03 16:36:52.430588+00	2025-12-03 16:36:52.430588+00	HEDGE_FUND_ACCOUNTING	FUND_SERVICES	\N	t	{}	ungoverned
3edd818d-d0c5-40e5-b497-411d69ae701f	Hedge Fund TA	Transfer agency services for hedge funds	2025-12-03 16:36:52.430588+00	2025-12-03 16:36:52.430588+00	HEDGE_FUND_TA	TRANSFER_AGENCY	\N	t	{}	ungoverned
c0658157-8201-4f9e-9e7d-34ba91a9aed8	Investor Register	\N	2025-12-03 16:29:18.55705+00	2025-12-03 16:29:18.55705+00	INVESTOR_REG	Transfer Agency	\N	t	{investor_services,regulatory}	ungoverned
42c49225-1a06-4416-99f0-ae89cebd8f8f	Asset Safekeeping	Secure custody of financial assets	2025-11-16 10:53:19.11495+00	2025-11-16 10:53:19.11495+00	SAFEKEEPING	Custody	\N	t	{core,regulatory}	ungoverned
22a021c0-e169-46cd-b4da-41c9b2c1cade	Cash Management	Cash forecasting and management	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	CASH_MGMT	Treasury	\N	t	{core}	ungoverned
b1b1cf13-369a-448e-aae6-24dfbb6ed739	Trade Settlement	Multi-market trade settlement	2025-11-16 10:53:19.11495+00	2025-11-16 10:53:19.11495+00	SETTLEMENT	Settlement	\N	t	{core}	ungoverned
0e12b362-c4e1-47d2-90f4-51ca0fddc1bf	Client Reporting	Regulatory and client reporting	2025-11-16 10:53:19.11495+00	2025-11-16 10:53:19.11495+00	REPORTING	reporting	\N	t	{reporting,regulatory}	ungoverned
a41be122-5c04-4944-9976-fdc656e25578	Corporate Actions	Corporate action processing and elections	2025-11-16 10:53:19.11495+00	2025-11-16 10:53:19.11495+00	CORP_ACTIONS	Operations	\N	t	{corporate_actions}	ungoverned
7c536c3f-d475-4fa1-b665-f05e3dbd4e45	NAV Calculation	Daily/periodic NAV calculation	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	NAV_CALC	Valuation	\N	t	{core,valuation}	ungoverned
e75cef50-8f2e-4260-8a18-4f63cab526dc	Regulatory Reporting	\N	2025-12-03 16:28:07.8773+00	2025-12-03 16:28:07.8773+00	REG_REPORTING	Reporting	\N	t	{reporting,regulatory}	ungoverned
71084961-a94f-438f-a5b7-bd7b5a425469	Investor Accounting	Shareholder servicing and transfer agency	2025-11-30 16:56:19.247083+00	2025-11-30 16:56:19.247083+00	INVESTOR_ACCT	Accounting	\N	t	{investor_services}	ungoverned
\.


--
-- PostgreSQL database dump complete
--

\unrestrict FGaAzPiFXh7NgkxdzCPJRsyOgaGVsyeVbbZ4JVwFAR2746CirgIzZTxKLzyMfaG
