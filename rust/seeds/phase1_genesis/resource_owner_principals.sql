--
-- PostgreSQL database dump
--

\restrict jTufSS7UnwcNcxcUml0kWmKlWyRsy895mMmw46XbSUEtyCk31uu3MjdYWYIkO1y

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
-- Data for Name: resource_owner_principals; Type: TABLE DATA; Schema: ob-poc; Owner: adamtc007
--

COPY "ob-poc".resource_owner_principals (owner_principal_fqn, owner_system, display_name, dispatch_endpoint, status, metadata, created_at, updated_at, principal_kind, principal_capabilities, dispatch_enabled) FROM stdin;
resource_owner:IAM	IAM	IAM	\N	active	{}	2026-05-05 13:26:36.947225+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:TRADING	TRADING	TRADING	\N	active	{}	2026-05-05 13:25:43.664522+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:Fund Services	Fund Services	Fund Services	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:REFINITIV	REFINITIV	REFINITIV	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:BLOOMBERG	BLOOMBERG	BLOOMBERG	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:ICE	ICE	ICE	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:MARKIT	MARKIT	MARKIT	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:SWIFT	SWIFT	SWIFT	\N	active	{}	2026-05-05 13:28:16.453684+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:DTCC	DTCC	DTCC	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:Middle Office	Middle Office	Middle Office	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:CLIENT	CLIENT	CLIENT	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:CUSTODY	CUSTODY	CUSTODY	\N	active	{}	2026-05-05 13:27:30.138135+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:BNY	BNY	BNY	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:API_GATEWAY	API_GATEWAY	API_GATEWAY	\N	active	{}	2026-05-05 13:28:16.456089+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:Technology	Technology	Technology	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
resource_owner:Operations	Operations	Operations	\N	active	{}	2026-05-05 12:59:24.429336+01	2026-05-19 16:26:17.078956+01	resource_owner	["resource_owner"]	t
\.


--
-- PostgreSQL database dump complete
--

\unrestrict jTufSS7UnwcNcxcUml0kWmKlWyRsy895mMmw46XbSUEtyCk31uu3MjdYWYIkO1y
