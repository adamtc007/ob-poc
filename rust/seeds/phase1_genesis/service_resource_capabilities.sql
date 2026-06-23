--
-- PostgreSQL database dump
--

\restrict cfM8B5dnfF4dLRBNbQzezVd4JT59HKvfNn2UthbDYnP3Jp30mVemXCzS3UCgD4X

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
-- Data for Name: service_resource_capabilities; Type: TABLE DATA; Schema: ob-poc; Owner: adamtc007
--

COPY "ob-poc".service_resource_capabilities (capability_id, service_id, resource_id, supported_options, priority, cost_factor, performance_rating, resource_config, is_active, is_required) FROM stdin;
9d7cdaf3-5f79-44fc-9dd6-7323de472dcf	b1b1cf13-369a-448e-aae6-24dfbb6ed739	4fd204b4-1c8f-4284-a61c-55618aedc522	{"speed": ["T0", "T1", "T2"], "markets": ["US_EQUITY"]}	100	1.0000	\N	\N	t	t
bf2efffc-b7ca-490f-994d-f9cb5f43ba9f	b1b1cf13-369a-448e-aae6-24dfbb6ed739	8cb7463a-4aff-4280-b41f-e7f76ccd76bd	{"speed": ["T1", "T2"], "markets": ["EU_EQUITY"]}	90	1.0000	\N	\N	t	t
0987a6ea-2940-4e66-92f5-6720cb742988	b1b1cf13-369a-448e-aae6-24dfbb6ed739	618d6312-85a7-437e-955a-5cd7131ce5c5	{"speed": ["T2"], "markets": ["APAC_EQUITY"]}	80	1.0000	\N	\N	t	t
bce337f8-740c-4340-b38a-c5d38b0a9c44	42c49225-1a06-4416-99f0-ae89cebd8f8f	f9a21fa6-0f95-4d65-9a7f-27485e31f5d2	{}	100	1.0000	\N	\N	t	t
6ef3b741-f871-4465-9885-76c5db338f2f	b1b1cf13-369a-448e-aae6-24dfbb6ed739	c58a0cb0-c32b-4e48-81ff-0f7dc2ae3bd2	{}	100	1.0000	\N	\N	t	t
a81a1180-3217-43e1-9689-708605e3db50	b1b1cf13-369a-448e-aae6-24dfbb6ed739	0891258b-18f8-4f3d-b0e1-ac99f8e722c2	{}	100	1.0000	\N	\N	t	t
0947b4d6-8a63-41c3-9cc3-c7b5b51e8abe	a41be122-5c04-4944-9976-fdc656e25578	209ab7d0-631c-4735-94c6-a804df0c671b	{}	100	1.0000	\N	\N	t	t
03f85027-0b7b-48ac-a420-535534469233	631b59f4-7317-432c-a4b2-64c12c643cdd	0891258b-18f8-4f3d-b0e1-ac99f8e722c2	{}	100	1.0000	\N	\N	t	t
1c12cf21-3a7d-42f4-8a09-322d900c30ad	7c536c3f-d475-4fa1-b665-f05e3dbd4e45	35344e29-efe6-4d05-b220-75f5629d4067	{}	100	1.0000	\N	\N	t	t
84af9aed-2a9e-477e-9ee9-6de243efedb3	71084961-a94f-438f-a5b7-bd7b5a425469	4daf88ee-2a29-4991-8233-c60d3807c936	{}	100	1.0000	\N	\N	t	t
1e176bc8-6b7f-4e14-8e47-d8f9a628fdda	e4140ad9-9d91-46c1-913d-ef38a06a12c3	bb17d171-b981-42c5-bc76-d1ade20ef892	{}	100	1.0000	\N	\N	t	t
857b5725-2a94-43b4-aaa1-91c6e674b4c1	91f5e266-8802-4522-bbd2-8501831a5ccb	495c19ce-a6a9-41a3-8596-bf3c3f8bd582	{}	100	1.0000	\N	\N	t	t
8535585f-500d-44b9-97e4-f540a739b218	555cea6c-d32b-4490-8705-9a157f48644d	495c19ce-a6a9-41a3-8596-bf3c3f8bd582	{}	100	1.0000	\N	\N	t	t
\.


--
-- PostgreSQL database dump complete
--

\unrestrict cfM8B5dnfF4dLRBNbQzezVd4JT59HKvfNn2UthbDYnP3Jp30mVemXCzS3UCgD4X
