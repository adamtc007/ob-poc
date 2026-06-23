--
-- PostgreSQL database dump
--

\restrict jltJSvlo8zHsH6E073ZerlLD6L3d2aqqgDexm1U79qu3tmQn5V1GKX1WT485GvZ

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
-- Data for Name: products; Type: TABLE DATA; Schema: ob-poc; Owner: adamtc007
--

COPY "ob-poc".products (product_id, name, description, created_at, updated_at, product_code, product_category, regulatory_framework, min_asset_requirement, is_active, metadata, kyc_risk_rating, kyc_context, requires_kyc, product_family, effective_from, effective_to) FROM stdin;
15244192-0e29-4cd4-8d3b-ec19488ad814	Custody	\N	2025-12-03 16:23:47.757813+00	2025-12-03 16:23:47.757813+00	CUSTODY	custody	\N	\N	t	\N	HIGH	CUSTODY	t	custody_services	2026-02-08 14:14:10.326215+00	\N
3e027380-ca07-41bf-a9c8-66606f338065	Transfer Agency	\N	2025-12-03 16:23:47.757813+00	2025-12-03 16:23:47.757813+00	TRANSFER_AGENCY	fund_services	\N	\N	t	\N	MEDIUM	TRANSFER_AGENT	t	fund_services	2026-02-08 14:14:10.326215+00	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	Fund Accounting	\N	2025-12-03 16:23:47.757813+00	2025-12-03 16:23:47.757813+00	FUND_ACCOUNTING	fund_services	\N	\N	t	\N	MEDIUM	CUSTODY	t	fund_services	2026-02-08 14:14:10.326215+00	\N
723ebb4f-bef3-4087-bebc-bd97f74aaba6	Markets FX	\N	2025-12-03 16:23:47.757813+00	2025-12-03 16:23:47.757813+00	MARKETS_FX	markets	\N	\N	t	\N	\N	\N	t	markets	2026-02-08 14:14:10.326215+00	\N
2b59406a-3204-4376-a7fc-71e5f6e9d6a0	Middle Office	\N	2025-12-03 16:23:47.757813+00	2025-12-03 16:23:47.757813+00	MIDDLE_OFFICE	operations	\N	\N	t	\N	\N	\N	t	middle_office	2026-02-08 14:14:10.326215+00	\N
051c88a0-ac42-4ec5-af5a-46b08500383e	Collateral Management	\N	2025-12-03 16:23:47.757813+00	2025-12-03 16:23:47.757813+00	COLLATERAL_MGMT	collateral	\N	\N	t	\N	\N	\N	t	collateral	2026-02-08 14:14:10.326215+00	\N
c740519b-d7bb-4887-b8f5-4c97cba6fa77	Alternatives	Alternative Investment Services	2025-12-03 16:36:52.430588+00	2025-12-03 16:36:52.430588+00	ALTS	INVESTMENT_SERVICES	\N	\N	t	\N	\N	\N	t	alternatives	2026-02-08 14:14:10.326215+00	\N
019c2e31-9f60-7ceb-b68e-85d1f5aa6f44	CUSTODY	Custody Services	2026-02-05 14:25:29.952591+00	2026-05-03 11:51:19.436001+01	\N	CORE	\N	\N	f	\N	\N	\N	t	\N	2026-02-08 14:14:10.326215+00	\N
019c2e31-9f64-7c04-b046-3e34220f8d7f	FUND_ACCOUNTING	Fund Accounting Services	2026-02-05 14:25:29.95664+00	2026-05-03 11:51:19.436001+01	\N	CORE	\N	\N	f	\N	\N	\N	t	\N	2026-02-08 14:14:10.326215+00	\N
\.


--
-- PostgreSQL database dump complete
--

\unrestrict jltJSvlo8zHsH6E073ZerlLD6L3d2aqqgDexm1U79qu3tmQn5V1GKX1WT485GvZ
