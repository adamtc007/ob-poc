--
-- PostgreSQL database dump
--

\restrict JRuP9fcixpLGoiC0RZZZxtBp61aGuaFYqX6c0MtLRneSDmDxkKdLy07SgqvaG2I

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
-- Data for Name: product_services; Type: TABLE DATA; Schema: ob-poc; Owner: adamtc007
--

COPY "ob-poc".product_services (product_id, service_id, is_mandatory, is_default, display_order, configuration) FROM stdin;
15244192-0e29-4cd4-8d3b-ec19488ad814	42c49225-1a06-4416-99f0-ae89cebd8f8f	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	a41be122-5c04-4944-9976-fdc656e25578	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	631b59f4-7317-432c-a4b2-64c12c643cdd	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	24c54be5-c507-45ce-9955-b52212d09b95	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	22a021c0-e169-46cd-b4da-41c9b2c1cade	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	d2d71198-dd5b-4f66-9547-ff76bf242e0a	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	4a560fb4-b635-4f84-a4ad-fba24b08818a	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	3fcc8612-d9e0-4346-a2d4-a4f95451cf87	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	045b5f42-b484-406f-a8f4-c44a3281e13f	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	7c536c3f-d475-4fa1-b665-f05e3dbd4e45	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	e4140ad9-9d91-46c1-913d-ef38a06a12c3	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	096f3dc8-3619-46ea-b2ba-a9a41c79bdcb	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	6f3b44ff-bbbf-4f61-a368-3436aaac6acd	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	e64867f9-67e4-4288-96ae-76df27e52376	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	ff28326c-c9ac-4752-b704-339a4263e17a	f	f	\N	\N
7d263b9a-8918-47d4-b469-c1d4fc84b529	e75cef50-8f2e-4260-8a18-4f63cab526dc	f	f	\N	\N
3e027380-ca07-41bf-a9c8-66606f338065	71084961-a94f-438f-a5b7-bd7b5a425469	f	f	\N	\N
3e027380-ca07-41bf-a9c8-66606f338065	c0658157-8201-4f9e-9e7d-34ba91a9aed8	f	f	\N	\N
3e027380-ca07-41bf-a9c8-66606f338065	e559f68b-120f-4a4e-9c09-af5793e1c860	f	f	\N	\N
3e027380-ca07-41bf-a9c8-66606f338065	911ed95b-97a1-4dfd-95f8-08c038e8ebe5	f	f	\N	\N
3e027380-ca07-41bf-a9c8-66606f338065	c50cda37-ad92-4437-bf72-18d12302da1f	f	f	\N	\N
2b59406a-3204-4376-a7fc-71e5f6e9d6a0	0e12b362-c4e1-47d2-90f4-51ca0fddc1bf	f	f	\N	\N
2b59406a-3204-4376-a7fc-71e5f6e9d6a0	91f5e266-8802-4522-bbd2-8501831a5ccb	f	f	\N	\N
2b59406a-3204-4376-a7fc-71e5f6e9d6a0	555cea6c-d32b-4490-8705-9a157f48644d	f	f	\N	\N
2b59406a-3204-4376-a7fc-71e5f6e9d6a0	e75cef50-8f2e-4260-8a18-4f63cab526dc	f	f	\N	\N
051c88a0-ac42-4ec5-af5a-46b08500383e	f4ad454a-4479-4bec-b922-a7f45922dc9d	f	f	\N	\N
051c88a0-ac42-4ec5-af5a-46b08500383e	2f43e57d-1de8-462a-bf6c-7460f4dfa3a1	f	f	\N	\N
723ebb4f-bef3-4087-bebc-bd97f74aaba6	449435fb-6b07-4c46-b0f5-77bee288b305	f	f	\N	\N
c740519b-d7bb-4887-b8f5-4c97cba6fa77	511d2ddf-1509-4627-ad78-fdf94fb7115d	f	f	\N	\N
c740519b-d7bb-4887-b8f5-4c97cba6fa77	3edd818d-d0c5-40e5-b497-411d69ae701f	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	e75cef50-8f2e-4260-8a18-4f63cab526dc	f	f	\N	\N
15244192-0e29-4cd4-8d3b-ec19488ad814	b1b1cf13-369a-448e-aae6-24dfbb6ed739	t	t	\N	\N
\.


--
-- PostgreSQL database dump complete
--

\unrestrict JRuP9fcixpLGoiC0RZZZxtBp61aGuaFYqX6c0MtLRneSDmDxkKdLy07SgqvaG2I
