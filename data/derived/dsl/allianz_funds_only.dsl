;; ============================================================================
;; Allianz Global Investors - Full ETL Load
;; Generated: 2025-12-28T22:35:37.672185
;; ============================================================================
;;
;; GLEIF entities: 2
;; Subsidiaries: 2
;; Funds: 671
;;

;; ======================================================================
;; PHASE 1: Ownership Chain (GLEIF)
;; ======================================================================

(entity.ensure-limited-company
  :name "Allianz SE"
  :jurisdiction "DE"
  :company-number "HRB 164232"
  :as @allianz_se)

(entity.ensure-limited-company
  :name "Allianz Global Investors GmbH"
  :jurisdiction "DE"
  :company-number "HRB 9340"
  :as @allianz_global_investors_gmbh)

;; Subsidiaries
(entity.ensure-limited-company
  :name "ALLIANZ CAPITAL PARTNERS OF AMERICA LLC"
  :jurisdiction "US"
  :company-number "3600054"
  :as @allianz_capital_partner_dd06e2)

(entity.ensure-limited-company
  :name "アリアンツ・グローバル・インベスターズ・ジャパン株式会社"
  :jurisdiction "JP"
  :company-number "0104-01-053740"
  :as @entity_8b14751a4e)

;; ======================================================================
;; PHASE 2: Ownership Relationships
;; ======================================================================

(ubo.add-ownership
  :owner-entity-id @allianz_se
  :owned-entity-id @allianz_global_investors_gmbh
  :ownership-type "DIRECT"
  :percentage 100.0)

(ubo.add-ownership
  :owner-entity-id @allianz_global_investors_gmbh
  :owned-entity-id @allianz_capital_partner_dd06e2
  :ownership-type "DIRECT"
  :percentage 100.0)

(ubo.add-ownership
  :owner-entity-id @allianz_global_investors_gmbh
  :owned-entity-id @entity_8b14751a4e
  :ownership-type "DIRECT"
  :percentage 100.0)

;; ======================================================================
;; PHASE 3: CBU (Client Business Unit)
;; ======================================================================

(cbu.ensure
  :name "Allianz Global Investors"
  :jurisdiction "DE"
  :as @cbu_agi)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_investors_gmbh
  :role "MANAGEMENT_COMPANY")

;; ======================================================================
;; PHASE 4: Funds (671 total)
;; ======================================================================

(fund.ensure-umbrella
  :name "Allianz Income and Growth"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_income_and_growth_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_ch
  :role "FUND")



















































;; ... and 27 more share classes (truncated)

(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_bca879)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_bca879
  :role "FUND")






























(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_c44d45)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_c44d45
  :role "FUND")

























(fund.ensure-umbrella
  :name "Allianz US Equity Fund"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_fund_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_ch
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz Asian Multi Income Plus"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_asian_multi_inc_b7e1c2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_multi_inc_b7e1c2
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Best Styles Europe Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_abb706)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_abb706
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Strategy 50"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_strategy_50_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_50_ch
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Smart Energy"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_smart_energy_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_smart_energy_ch
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth Select"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_f0ab92)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_f0ab92
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Global Credit"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_credit_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_ch
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz China Future Technologies"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_china_future_te_98f65d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_future_te_98f65d
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Global Intelligent Cities Income"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_intellig_571657)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_intellig_571657
  :role "FUND")























(fund.ensure-umbrella
  :name "Allianz Global Artificial Intelligence"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_artifici_97071a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_97071a
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Oriental Income"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_oriental_income_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_ch
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz US High Yield"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_us_high_yield_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_ch
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Global Floating Rate Notes Plus"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_floating_e0227f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_e0227f
  :role "FUND")


















































(fund.ensure-umbrella
  :name "Allianz Multi Asset Long / Short"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_lon_d37ce1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_d37ce1
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_917243)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_917243
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Global Sustainability"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_sustaina_5cf32a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_5cf32a
  :role "FUND")



































(fund.ensure-umbrella
  :name "Allianz Euro High Yield Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_e33a67)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_e33a67
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Strategic Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_strategic_bond_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_ch
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Global"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__ae11ad)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__ae11ad
  :role "FUND")


(fund.ensure-subfund
  :name "Fondak"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @fondak_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondak_ch
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz US Investment Grade Credit"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_us_investment_g_ada42c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_ada42c
  :role "FUND")





































(fund.ensure-umbrella
  :name "Allianz Japan Smaller Companies Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_japan_smaller_c_0b8d2a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_0b8d2a
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Credit Opportunities"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_credit_opportun_d5742f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_d5742f
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz China Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_china_equity_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_ch
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Global Diversified Credit"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_350665)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_350665
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Euro Government Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_government_5f58d1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_government_5f58d1
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Europe Equity Value"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_value_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_value_ch
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Thematica"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_thematica_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_ch
  :role "FUND")

































(fund.ensure-umbrella
  :name "Allianz Global Water"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_water_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_ch
  :role "FUND")
































(fund.ensure-umbrella
  :name "Allianz Best Styles Europe Equity SRI"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_a471c7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_a471c7
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Volatility Strategy Fund"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_volatility_stra_e2fdf0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_e2fdf0
  :role "FUND")













(fund.ensure-subfund
  :name "Allianz Euro Oblig Court Terme ISR"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_oblig_cour_bb34f6)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_oblig_cour_bb34f6
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Flexi Asia Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_flexi_asia_bond_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_asia_bond_ch
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 75"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_612093)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_612093
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Short Duration"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__610108)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__610108
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz HKD Income"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_hkd_income_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hkd_income_ch
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity SRI"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_43e358)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_43e358
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Floating Rate Notes Plus"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_floating_rate_n_3b823a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_3b823a
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Hong Kong Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_hong_kong_equity_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hong_kong_equity_ch
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Equity Growth"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_g_e0b93f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_g_e0b93f
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Euroland Equity Growth"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euroland_equity_656c2e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_656c2e
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Asian Small Cap Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_asian_small_cap_72d11a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_small_cap_72d11a
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Emerging Europe Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_europe_94d452)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_94d452
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Europe Small Cap Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_europe_small_ca_3bae47)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_3bae47
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Clean Planet"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_clean_planet_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_clean_planet_ch
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz US Short Duration High Income Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_us_short_durati_48389e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_48389e
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Dynamic Commodities"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_commodities_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_ch
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz India Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_india_equity_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_ch
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz European Equity Dividend"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_european_equity_ab9019)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_ab9019
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Green Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_green_bond_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_ch
  :role "FUND")
























(fund.ensure-umbrella
  :name "Allianz Food Security"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_food_security_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_ch
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Europe Equity SRI"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_sri_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_sri_ch
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Global Equity Unconstrained"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_u_732bb8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_732bb8
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity SRI"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_dad7c1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_dad7c1
  :role "FUND")












(fund.ensure-subfund
  :name "Allianz Euro Rentenfonds"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_rentenfonds_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_rentenfonds_ch
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz All China Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_all_china_equity_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_ch
  :role "FUND")





























(fund.ensure-umbrella
  :name "Allianz Renminbi Fixed Income"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_renminbi_fixed__b7214c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_renminbi_fixed__b7214c
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Euro Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_bond_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_ch
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Pet and Animal Wellbeing"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_pet_and_animal__86eb53)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pet_and_animal__86eb53
  :role "FUND")























(fund.ensure-umbrella
  :name "Allianz Positive Change"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_positive_change_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_positive_change_ch
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Global Diversified Dividend"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_74e064)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_74e064
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 50"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_5d4893)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_5d4893
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Dynamic Asian High Yield Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_asian_h_bab7cf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_bab7cf
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz Global Hi-Tech Growth"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_hi_tech__3afac6)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__3afac6
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Global Metals and Mining"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_metals_a_ddf260)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_ddf260
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz China A-Shares"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_china_a_shares_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_ch
  :role "FUND")



























(fund.ensure-subfund
  :name "Allianz Nebenwerte Deutschland"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_nebenwerte_deut_02420e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_nebenwerte_deut_02420e
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Euro High Yield Defensive"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_b6dd78)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_b6dd78
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Dynamic Allocation Plus Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_allocat_5315d2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_allocat_5315d2
  :role "FUND")









(fund.ensure-subfund
  :name "ALLIANZ EURO HIGH YIELD"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_ch
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Treasury Short Term Plus Euro"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_treasury_short__92ecf1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__92ecf1
  :role "FUND")








(fund.ensure-subfund
  :name "Allianz Vermögensbildung Europa"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_verm_gensbildun_5faa5f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_verm_gensbildun_5faa5f
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Japan Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_japan_equity_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_ch
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Global High Yield"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_high_yield_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_ch
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz US Large Cap Value"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_us_large_cap_value_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_ch
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz China A Opportunities"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_china_a_opportu_af25f3)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_af25f3
  :role "FUND")


















(fund.ensure-subfund
  :name "Allianz Europazins"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_europazins_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europazins_ch
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Small Cap Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_small_ca_04a65a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_04a65a
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz Asia Pacific Income"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_asia_pacific_income_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_pacific_income_ch
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz GEM Equity High Dividend"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_gem_equity_high_3eff39)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_3eff39
  :role "FUND")





















(fund.ensure-umbrella
  :name "ALLIANZ VALEURS DURABLES"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_valeurs_durables_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_valeurs_durables_ch
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Global Equity Insights"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_i_280bff)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_280bff
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Global Opportunistic Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_opportun_b00a84)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_b00a84
  :role "FUND")































(fund.ensure-umbrella
  :name "Allianz Emerging Markets SRI Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_22fcdb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_22fcdb
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Convertible Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_convertible_bond_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_convertible_bond_ch
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 15"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_8dd69a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_8dd69a
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz SDG Euro Credit"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_sdg_euro_credit_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_euro_credit_ch
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity Opportunities"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_2ff02c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_2ff02c
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Total Return Asian Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_total_return_as_265b3a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_265b3a
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond II"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_43523c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_43523c
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz German Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_german_equity_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_ch
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Euro Credit SRI"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_credit_sri_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_ch
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Best Styles US Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__0f9660)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__0f9660
  :role "FUND")






















(fund.ensure-umbrella
  :name "Allianz Cyber Security"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_cyber_security_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_ch
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Emerging Markets Corporate Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_824fc1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_824fc1
  :role "FUND")














(fund.ensure-subfund
  :name "Allianz Fonds Schweiz"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_fonds_schweiz_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fonds_schweiz_ch
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Global Allocation Opportunities"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_global_allocati_3e26ee)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_allocati_3e26ee
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Global Aggregate"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__c1eb7a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__c1eb7a
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Emerging Markets Sovereign Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_41d33f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_41d33f
  :role "FUND")


























(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Euro"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__183508)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__183508
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 30"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_86d083)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_86d083
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Best Styles Euroland Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_319e1c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_319e1c
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Euro Inflation-linked Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_euro_inflation__f45e79)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_inflation__f45e79
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz High Dividend Asia Pacific Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_high_dividend_a_c48a9d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_high_dividend_a_c48a9d
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Credit Opportunities Plus"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_credit_opportun_b4a469)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_b4a469
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Enhanced Short Term Euro"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_enhanced_short__80e1de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_enhanced_short__80e1de
  :role "FUND")














(fund.ensure-subfund
  :name "Allianz Internationaler Rentenfonds"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_internationaler_3a0f27)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_internationaler_3a0f27
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Best Styles US Small Cap Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__5f03ae)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__5f03ae
  :role "FUND")














(fund.ensure-subfund
  :name "Allianz Diversified Swiss Equity"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_diversified_swi_7818a4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_diversified_swi_7818a4
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz Swiss Bond"
  :jurisdiction "CH"
  :regulatory-status "UCITS"
  :as @allianz_swiss_bond_ch)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_swiss_bond_ch
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Money Market US $"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_money_market_us_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_money_market_us_de
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz US Equity Fund"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_fund_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_de
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz Best Styles Pacific Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_pac_d697a9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_pac_d697a9
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth Select"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_64b000)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_64b000
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Capital Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_capital_plus_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_de
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Green Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_green_bond_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_de
  :role "FUND")
























(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Global Aggregate"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__288508)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__288508
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Asian Small Cap Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_asian_small_cap_fc28b4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_small_cap_fc28b4
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz China Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_china_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_de
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Global Water"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_water_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_de
  :role "FUND")
































(fund.ensure-umbrella
  :name "Allianz Global Artificial Intelligence"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_artifici_6d30d4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_6d30d4
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Best Styles Europe Equity SRI"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_732aa8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_732aa8
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz US Short Duration High Income Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_short_durati_60f3c7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_60f3c7
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Food Security"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_food_security_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_de
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz US Investment Grade Credit"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_investment_g_bd539e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_bd539e
  :role "FUND")





































(fund.ensure-umbrella
  :name "Allianz Total Return Asian Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_total_return_as_a14f5b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_a14f5b
  :role "FUND")















(fund.ensure-subfund
  :name "PremiumMandat Balance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @premiummandat_balance_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_balance_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Best Styles Global AC Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_e9395e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_e9395e
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity SRI"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_9d8926)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_9d8926
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Hong Kong Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_hong_kong_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hong_kong_equity_de
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Opportunistic Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_opportun_c2bf67)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_c2bf67
  :role "FUND")































(fund.ensure-umbrella
  :name "Allianz Emerging Markets Short Duration Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_9ada8b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_9ada8b
  :role "FUND")











(fund.ensure-subfund
  :name "PremiumMandat Dynamik"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @premiummandat_dynamik_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_dynamik_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_5600e2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_5600e2
  :role "FUND")

























(fund.ensure-umbrella
  :name "Allianz Positive Change"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_positive_change_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_positive_change_de
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Strategy 75"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_75_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_75_de
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Alternative Investment Strategies"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_alternative_inv_25ce00)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_alternative_inv_25ce00
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Mobil-Fonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_mobil_fonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_mobil_fonds_de
  :role "FUND")


(fund.ensure-subfund
  :name "Concentra"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @concentra_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @concentra_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global High Yield"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_high_yield_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_de
  :role "FUND")














(fund.ensure-subfund
  :name "Plusfonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @plusfonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @plusfonds_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Volatility Strategy Fund"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_volatility_stra_bded97)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_bded97
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz US High Yield"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_high_yield_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_de
  :role "FUND")




















(fund.ensure-subfund
  :name "Allianz Multi Manager Global Balanced"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_multi_manager_g_feb2cd)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_manager_g_feb2cd
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Strategie 2036 Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategie_2036_plus_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategie_2036_plus_de
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Strategie 2031 Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategie_2031_plus_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategie_2031_plus_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Europe Small Cap Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europe_small_ca_b4a933)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_b4a933
  :role "FUND")









(fund.ensure-subfund
  :name "Anlagestruktur 1"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @anlagestruktur_1_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @anlagestruktur_1_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz India Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_india_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_de
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Best Styles Europe Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_cb3a69)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_cb3a69
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Euro Inflation-linked Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_inflation__5e8d9b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_inflation__5e8d9b
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Dynamic Allocation Plus Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_allocat_3f8418)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_allocat_3f8418
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz High Dividend Asia Pacific Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_high_dividend_a_f4cffe)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_high_dividend_a_f4cffe
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity SRI"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_706f87)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_706f87
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_802b05)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_802b05
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Global Equity Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_g_fb4bd7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_g_fb4bd7
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Convertible Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_convertible_bond_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_convertible_bond_de
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz China A Opportunities"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_china_a_opportu_cfe532)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_cfe532
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Innovation Souveraineté Européenne"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_innovation_souv_0ca2e2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_innovation_souv_0ca2e2
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz European Equity Dividend"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_european_equity_0af205)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_0af205
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Strategy 50"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_50_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_50_de
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Strategy 15"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_15_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_15_de
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz China A-Shares"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_china_a_shares_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_de
  :role "FUND")



























(fund.ensure-subfund
  :name "Allianz Fonds Schweiz"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fonds_schweiz_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fonds_schweiz_de
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Flexi Asia Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_flexi_asia_bond_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_asia_bond_de
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz SDG Global Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_sdg_global_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_global_equity_de
  :role "FUND")









(fund.ensure-subfund
  :name "Fondra"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @fondra_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondra_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Euro High Yield Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_0241ac)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_0241ac
  :role "FUND")
















(fund.ensure-subfund
  :name "Fondis"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @fondis_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondis_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Thematica"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_thematica_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_de
  :role "FUND")

































(fund.ensure-subfund
  :name "Flexible Portfolio"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @flexible_portfolio_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @flexible_portfolio_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Credit"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_credit_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_de
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Global Hi-Tech Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_hi_tech__4c7171)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__4c7171
  :role "FUND")





(fund.ensure-subfund
  :name "Allianz Internationaler Rentenfonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_internationaler_55bf7c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_internationaler_55bf7c
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Euroland Equity Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euroland_equity_b335db)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_b335db
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Euro"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__a8b430)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__a8b430
  :role "FUND")

















(fund.ensure-subfund
  :name "Allianz Nebenwerte Deutschland"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_nebenwerte_deut_d8c4bf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_nebenwerte_deut_d8c4bf
  :role "FUND")








(fund.ensure-subfund
  :name "ALLIANZ ACTIONS EURO CONVICTIONS"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_actions_euro_co_ca2053)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_actions_euro_co_ca2053
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz German Small and Micro Cap"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_german_small_an_c83f30)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_small_an_c83f30
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Global Government Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_governme_5f24b9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_governme_5f24b9
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Diversified Dividend"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_80f3c5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_80f3c5
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Global Dividend"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_dividend_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_dividend_de
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Multi Asset Long / Short"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_lon_822620)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_822620
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Global Aggregate Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_aggregat_46f451)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_aggregat_46f451
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Balanced Income and Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_balanced_income_e8d1a0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_balanced_income_e8d1a0
  :role "FUND")



















(fund.ensure-umbrella
  :name "Allianz Cyber Security"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_cyber_security_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_de
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz All China Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_all_china_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_de
  :role "FUND")





























(fund.ensure-subfund
  :name "VermögensManagement Stabilität"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_sta_80d8b7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sta_80d8b7
  :role "FUND")


(fund.ensure-subfund
  :name "CONVEST 21 VL"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @convest_21_vl_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @convest_21_vl_de
  :role "FUND")


(fund.ensure-subfund
  :name "Best-in-One"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @best_in_one_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @best_in_one_de
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Adiverba"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_adiverba_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_adiverba_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Oriental Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_oriental_income_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_de
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Pet and Animal Wellbeing"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_pet_and_animal__3d0740)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pet_and_animal__3d0740
  :role "FUND")























(fund.ensure-subfund
  :name "Allianz PIMCO High Yield Income Fund"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_pimco_high_yiel_0b4682)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pimco_high_yiel_0b4682
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 75"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_138a08)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_138a08
  :role "FUND")




























(fund.ensure-subfund
  :name "Allianz Fondsvorsorge 1977-1996"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fondsvorsorge_1_5e935b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_5e935b
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Fondsvorsorge 1967-1976"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fondsvorsorge_1_63aacd)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_63aacd
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Credit Opportunities"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_credit_opportun_a7b52a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_a7b52a
  :role "FUND")












(fund.ensure-subfund
  :name "Allianz Fondsvorsorge 1957-1966"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fondsvorsorge_1_4d2c32)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_4d2c32
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Fondsvorsorge 1952-1956"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fondsvorsorge_1_278e90)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_278e90
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz US Large Cap Value"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_large_cap_value_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_de
  :role "FUND")















(fund.ensure-subfund
  :name "Allianz Fondsvorsorge 1947-1951"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fondsvorsorge_1_f599e3)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_f599e3
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Asian Multi Income Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_asian_multi_inc_a5f318)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_multi_inc_a5f318
  :role "FUND")
















(fund.ensure-subfund
  :name "PremiumMandat Konservativ"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @premiummandat_konservativ_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_konservativ_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Better World Defensive"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_better_world_de_72ac97)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_de_72ac97
  :role "FUND")








(fund.ensure-subfund
  :name "Allianz Biotechnologie"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_biotechnologie_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_biotechnologie_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Emerging Markets Sovereign Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_bf00da)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_bf00da
  :role "FUND")


























(fund.ensure-umbrella
  :name "Allianz Income and Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_income_and_growth_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_de
  :role "FUND")



















































;; ... and 27 more share classes (truncated)

(fund.ensure-subfund
  :name "Allianz Wachstum Europa"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_wachstum_europa_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_wachstum_europa_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Euro Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_bond_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_de
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz SDG Euro Credit"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_sdg_euro_credit_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_euro_credit_de
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Global Sustainability"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_sustaina_cb8d54)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_cb8d54
  :role "FUND")



































(fund.ensure-umbrella
  :name "Allianz Better World Moderate"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_better_world_mo_e2d125)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_mo_e2d125
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Short Duration"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__626586)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__626586
  :role "FUND")
















(fund.ensure-subfund
  :name "Allianz Vermögensbildung Europa"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_verm_gensbildun_37d2b3)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_verm_gensbildun_37d2b3
  :role "FUND")




(fund.ensure-subfund
  :name "VermögensManagement Wachstumsländer Balance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_wac_6225a3)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_6225a3
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz European Micro Cap"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_european_micro_cap_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_micro_cap_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond II"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_e4dff9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_e4dff9
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz Europazins"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europazins_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europazins_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Best Styles US Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__a27487)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__a27487
  :role "FUND")






















(fund.ensure-subfund
  :name "money mate entschlossen"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @money_mate_entschlossen_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_entschlossen_de
  :role "FUND")


(fund.ensure-subfund
  :name "Industria"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @industria_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @industria_de
  :role "FUND")



(fund.ensure-subfund
  :name "VermögensManagement RenditeStars"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_ren_281227)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_281227
  :role "FUND")



(fund.ensure-subfund
  :name "money mate mutig"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @money_mate_mutig_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_mutig_de
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Informationstechnologie"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_informationstec_117281)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_informationstec_117281
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz Euro Rentenfonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_rentenfonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_rentenfonds_de
  :role "FUND")




(fund.ensure-subfund
  :name "money mate moderat"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @money_mate_moderat_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_moderat_de
  :role "FUND")


(fund.ensure-subfund
  :name "VermögensManagement Stars of Multi Asset"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_sta_bbdd39)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sta_bbdd39
  :role "FUND")




(fund.ensure-subfund
  :name "money mate defensiv"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @money_mate_defensiv_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_defensiv_de
  :role "FUND")


(fund.ensure-umbrella
  :name "IndexManagement Substanz"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @indexmanagement_substanz_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_substanz_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Global"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__609d3c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__609d3c
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Fonds Japan"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_fonds_japan_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fonds_japan_de
  :role "FUND")


(fund.ensure-umbrella
  :name "IndexManagement Balance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @indexmanagement_balance_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_balance_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Premium Champions"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_premium_champions_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_premium_champions_de
  :role "FUND")





(fund.ensure-subfund
  :name "Allianz Interglobal"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_interglobal_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_interglobal_de
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz Vermögensbildung Deutschland"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_verm_gensbildun_963420)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_verm_gensbildun_963420
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Better World Dynamic"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_better_world_dy_ec1947)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_dy_ec1947
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Renminbi Fixed Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_renminbi_fixed__ca9577)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_renminbi_fixed__ca9577
  :role "FUND")








(fund.ensure-umbrella
  :name "IndexManagement Wachstum"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @indexmanagement_wachstum_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_wachstum_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Diversified Credit"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_99f5ec)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_99f5ec
  :role "FUND")





















(fund.ensure-umbrella
  :name "IndexManagement Chance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @indexmanagement_chance_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_chance_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Climate Transition Europe"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_climate_transit_6b0587)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_6b0587
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Floating Rate Notes Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_floating_84e2a0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_84e2a0
  :role "FUND")


















































(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 50"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_f5b264)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_f5b264
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond III"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_5b29bf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_5b29bf
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Little Dragons"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_little_dragons_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_little_dragons_de
  :role "FUND")





(fund.ensure-subfund
  :name "Allianz US Large Cap Growth"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_large_cap_growth_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_growth_de
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz FinanzPlan 2050"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2050_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2050_de
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz Thesaurus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_thesaurus_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thesaurus_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Dynamic Commodities"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_commodities_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_de
  :role "FUND")








(fund.ensure-subfund
  :name "Allianz Global Equity Dividend"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_d_74e1e2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_d_74e1e2
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Treasury Short Term Plus Euro"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_treasury_short__c92eca)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__c92eca
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Capital Plus Global"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_capital_plus_global_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_global_de
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz Rohstofffonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_rohstofffonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_rohstofffonds_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Emerging Markets Select Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_8180a3)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_8180a3
  :role "FUND")

















(fund.ensure-subfund
  :name "Allianz Adifonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_adifonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_adifonds_de
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Emerging Markets SRI Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_54d2df)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_54d2df
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Asia Pacific Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_asia_pacific_income_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_pacific_income_de
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Smart Energy"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_smart_energy_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_smart_energy_de
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Global Metals and Mining"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_metals_a_28251b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_28251b
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Global Allocation Opportunities"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_allocati_246f28)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_allocati_246f28
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Europe Equity powered by Artificial Intelligence"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_p_dca05c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_p_dca05c
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Multi Asset Risk Control"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_ris_c20252)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_ris_c20252
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz US Equity powered by Artificial Intelligence"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_power_c72b45)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_power_c72b45
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Europe Equity SRI"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_sri_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_sri_de
  :role "FUND")









(fund.ensure-subfund
  :name "VermögensManagement Chance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_chance_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_chance_de
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Strategiefonds Stabilität"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategiefonds__b40bc5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__b40bc5
  :role "FUND")






(fund.ensure-subfund
  :name "VermögensManagement Wachstum"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_wac_f06600)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_f06600
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Equity powered by Artificial Intelligence"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_p_41c25a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_p_41c25a
  :role "FUND")




(fund.ensure-subfund
  :name "VermögensManagement Balance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_balance_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_balance_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_e4f6d6)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_e4f6d6
  :role "FUND")






























(fund.ensure-subfund
  :name "VermögensManagement Substanz"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_sub_4b4fd8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sub_4b4fd8
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Climate Transition Credit"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_climate_transit_94b15a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_94b15a
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Equity Insights"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_i_6379c2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_6379c2
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond IV"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_e97807)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_e97807
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Emerging Markets Corporate Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_46e198)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_46e198
  :role "FUND")














(fund.ensure-subfund
  :name "Allianz Strategiefonds Wachstum Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategiefonds__f0f85d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__f0f85d
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz Euro Cash"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_cash_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_cash_de
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Dynamic Asian High Yield Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_asian_h_b6c802)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_b6c802
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz Strategy4Life Europe 40"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategy4life_e_9e9531)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy4life_e_9e9531
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz GEM Equity High Dividend"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_gem_equity_high_480747)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_480747
  :role "FUND")





















(fund.ensure-subfund
  :name "MetallRente FONDS PORTFOLIO"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @metallrente_fonds_portfolio_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @metallrente_fonds_portfolio_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 30"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_e1cfb4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_e1cfb4
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Asia Ex China Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_asia_ex_china_e_8e9afe)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_ex_china_e_8e9afe
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz China Future Technologies"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_china_future_te_b31d00)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_future_te_b31d00
  :role "FUND")


















(fund.ensure-subfund
  :name "VermögensManagement Einkommen Europa"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_ein_a7d89d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ein_a7d89d
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Strategic Bond"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategic_bond_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_de
  :role "FUND")



























(fund.ensure-subfund
  :name "Fondak"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @fondak_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondak_de
  :role "FUND")






(fund.ensure-subfund
  :name "ALLIANZ EURO HIGH YIELD"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_de
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Global Equity Unconstrained"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_u_69d75a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_69d75a
  :role "FUND")











(fund.ensure-subfund
  :name "Allianz Stiftungsfonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_stiftungsfonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_stiftungsfonds_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 15"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_c39491)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_c39491
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Credit Opportunities Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_credit_opportun_f7d45b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_f7d45b
  :role "FUND")











(fund.ensure-subfund
  :name "ALLIANZ SECURICASH SRI"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_securicash_sri_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_securicash_sri_de
  :role "FUND")




(fund.ensure-subfund
  :name "SK Welt"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @sk_welt_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_welt_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Strategy Select 75"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_75_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_75_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Euro High Yield Defensive"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_7cb0a4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_7cb0a4
  :role "FUND")







(fund.ensure-subfund
  :name "SK Themen"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @sk_themen_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_themen_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Strategy Select 50"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_50_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_50_de
  :role "FUND")




(fund.ensure-subfund
  :name "Allianz Global Infrastructure ELTIF"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_infrastr_9764e4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_infrastr_9764e4
  :role "FUND")











(fund.ensure-subfund
  :name "SK Europa"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @sk_europa_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_europa_de
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz Strategiefonds Wachstum"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategiefonds__04f2c4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__04f2c4
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Systematic Enhanced US Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_systematic_enha_b84a95)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_systematic_enha_b84a95
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Europe Equity Value"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_value_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_value_de
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Best Styles Euroland Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_be7392)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_be7392
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Japan Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_japan_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_de
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Global Intelligent Cities Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_intellig_296a9a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_intellig_296a9a
  :role "FUND")























(fund.ensure-subfund
  :name "Allianz Strategiefonds Balance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_strategiefonds__8c37a8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__8c37a8
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz HKD Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_hkd_income_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hkd_income_de
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Enhanced Short Term Euro"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_enhanced_short__d2a16a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_enhanced_short__d2a16a
  :role "FUND")














(fund.ensure-subfund
  :name "VermögensManagement DividendenStars"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_div_986aed)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_div_986aed
  :role "FUND")





(fund.ensure-subfund
  :name "Allianz Euro Oblig Court Terme ISR"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_oblig_cour_887837)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_oblig_cour_887837
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Euro Credit SRI"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_credit_sri_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_de
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Global Small Cap Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_small_ca_31c0bb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_31c0bb
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz Best Styles US Small Cap Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__559c8a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__559c8a
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond I"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_d9f297)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_d9f297
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Emerging Europe Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_europe_e90d80)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_e90d80
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Floating Rate Notes Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_floating_rate_n_11ccb9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_11ccb9
  :role "FUND")















(fund.ensure-subfund
  :name "Allianz Wachstum Euroland"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_wachstum_euroland_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_wachstum_euroland_de
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Clean Planet"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_clean_planet_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_clean_planet_de
  :role "FUND")








(fund.ensure-subfund
  :name "VermögensManagement RentenStars"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_ren_204468)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_204468
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz AI Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_ai_income_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_ai_income_de
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz European Bond RC"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_european_bond_rc_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_bond_rc_de
  :role "FUND")





(fund.ensure-subfund
  :name "VermögensManagement AktienStars"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_akt_57df65)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_akt_57df65
  :role "FUND")




(fund.ensure-subfund
  :name "Allianz SGB Renten"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_sgb_renten_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sgb_renten_de
  :role "FUND")


(fund.ensure-subfund
  :name "PremiumStars Wachstum"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @premiumstars_wachstum_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiumstars_wachstum_de
  :role "FUND")


(fund.ensure-subfund
  :name "PremiumStars Chance"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @premiumstars_chance_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiumstars_chance_de
  :role "FUND")


(fund.ensure-subfund
  :name "NÜRNBERGER Euroland A"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @n_rnberger_euroland_a_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @n_rnberger_euroland_a_de
  :role "FUND")


(fund.ensure-subfund
  :name "Kapital Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @kapital_plus_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @kapital_plus_de
  :role "FUND")








(fund.ensure-subfund
  :name "Allianz Flexi Rentenfonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_flexi_rentenfonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_rentenfonds_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz German Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_german_equity_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_de
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global Income"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_income_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_income_de
  :role "FUND")
















(fund.ensure-subfund
  :name "Allianz Rentenfonds"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_rentenfonds_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_rentenfonds_de
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz European Autonomy"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_european_autonomy_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_autonomy_de
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Japan Smaller Companies Equity"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_japan_smaller_c_597e4a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_597e4a
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Euro Bond Short Term 1-3 Plus"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_euro_bond_short_62e269)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_short_62e269
  :role "FUND")




(fund.ensure-subfund
  :name "Allianz FinanzPlan 2025"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2025_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2025_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Bond Fund"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_global_bond_fund_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_bond_fund_de
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz FinanzPlan 2030"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2030_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2030_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity Opportunities"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_eda48d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_eda48d
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz FinanzPlan 2035"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2035_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2035_de
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz FinanzPlan 2040"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2040_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2040_de
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz FinanzPlan 2045"
  :jurisdiction "DE"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2045_de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2045_de
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Income and Growth"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_income_and_growth_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_gb
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_ab96b1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_ab96b1
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Strategic Bond Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_strategic_bond_fund_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_fund_gb
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Best Styles US Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__5c41bf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__5c41bf
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Oriental Income"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_oriental_income_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_gb
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Diversified Credit"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_9fd2be)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_9fd2be
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Japan Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_japan_equity_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_gb
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Dynamic Asian High Yield Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_asian_h_2680a0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_2680a0
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz US Equity Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_fund_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_gb
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz UK Listed Opportunities Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_uk_listed_oppor_142517)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_listed_oppor_142517
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz China A-Shares"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_china_a_shares_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_gb
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz All China Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_all_china_equity_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_gb
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Global Floating Rate Notes Plus"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_floating_a3d627)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_a3d627
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz China A Opportunities"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_china_a_opportu_c92b69)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_c92b69
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz US Short Duration High Income Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_us_short_durati_36c047)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_36c047
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_11f841)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_11f841
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Artificial Intelligence"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_artifici_bd8350)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_bd8350
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Best Styles Global AC Equity Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_e22c3c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_e22c3c
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz RiskMaster Growth Multi Asset Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_riskmaster_grow_6c8f61)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_riskmaster_grow_6c8f61
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Floating Rate Notes Plus"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_floating_rate_n_5894a5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_5894a5
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Cyber Security"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_cyber_security_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_gb
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Euro Credit SRI"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_euro_credit_sri_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_gb
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz RiskMaster Moderate Multi Asset Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_riskmaster_mode_cc4414)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_riskmaster_mode_cc4414
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Green Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_green_bond_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_gb
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Emerging Markets Short Duration Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_924592)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_924592
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz RiskMaster Conservative Multi Asset Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_riskmaster_cons_2ddc8a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_riskmaster_cons_2ddc8a
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Gilt Yield Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_gilt_yield_fund_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gilt_yield_fund_gb
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth Select"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_e60b3a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_e60b3a
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Water"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_water_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_gb
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Emerging Markets Select Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_69f19f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_69f19f
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Total Return Asian Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_total_return_as_7ee2e9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_7ee2e9
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Total Return Asian Equity Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_total_return_as_7ebc8b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_7ebc8b
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Continental European Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_continental_eur_13134a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_continental_eur_13134a
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz UK Government Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_uk_government_bond_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_government_bond_gb
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_ab5f02)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_ab5f02
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Emerging Markets SRI Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_8b44fc)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_8b44fc
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz UK Listed Equity Income Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_uk_listed_equit_6e3286)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_listed_equit_6e3286
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Emerging Markets Sovereign Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_3a3f5b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_3a3f5b
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_00e159)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_00e159
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Emerging Markets Corporate Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_33a53d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_33a53d
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Thematica"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_thematica_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_gb
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Strategic Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_strategic_bond_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_gb
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Strategy 15"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_strategy_15_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_15_gb
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Global Multi Sector Credit Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_multi_se_e9a8b8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_multi_se_e9a8b8
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz India Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_india_equity_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_gb
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Opportunistic Bond"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_opportun_b904b7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_b904b7
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Thematica Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_thematica_fund_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_fund_gb
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global Hi-Tech Growth"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_hi_tech__f051ce)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__f051ce
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Treasury Short Term Plus Euro"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_treasury_short__8ed62f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__8ed62f
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Index-Linked Gilt Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_index_linked_gi_ff9d6e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_index_linked_gi_ff9d6e
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Global Sustainability"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_sustaina_a1690c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_a1690c
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz European Equity Dividend"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_european_equity_2ea40e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_2ea40e
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz GEM Equity High Dividend"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_gem_equity_high_8c7f4d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_8c7f4d
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global Credit"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_credit_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_gb
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global High Yield"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_high_yield_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_gb
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Dividend"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_dividend_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_dividend_gb
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Multi Asset Long / Short"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_lon_d186ea)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_d186ea
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz US Investment Grade Credit"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_us_investment_g_c281a9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_c281a9
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Best Styles Euroland Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_79aeee)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_79aeee
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz China A-Shares Equity Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_china_a_shares__f0834a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares__f0834a
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Small Cap Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_global_small_ca_4079cc)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_4079cc
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Technology Trust PLC"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_technology_trus_9ddc55)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_technology_trus_9ddc55
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Food Security"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_food_security_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_gb
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz US Large Cap Value"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_us_large_cap_value_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_gb
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Best Styles Global AC Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_531a71)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_531a71
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Legacy Builder Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_legacy_builder_fund_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_legacy_builder_fund_gb
  :role "FUND")


(fund.ensure-subfund
  :name "The Brunner Investment Trust PLC"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @the_brunner_investment__f05e68)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @the_brunner_investment__f05e68
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Best Styles US Small Cap Equity"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__759246)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__759246
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Volatility Strategy Fund"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @allianz_volatility_stra_27a3d7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_27a3d7
  :role "FUND")


(fund.ensure-subfund
  :name "The Merchants Trust PLC"
  :jurisdiction "GB"
  :regulatory-status "UCITS"
  :as @the_merchants_trust_plc_gb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @the_merchants_trust_plc_gb
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_6d4cc0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_6d4cc0
  :role "FUND")

























(fund.ensure-umbrella
  :name "Allianz Euro Bond"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_euro_bond_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_ie
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Income and Growth"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_income_and_growth_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_ie
  :role "FUND")



















































;; ... and 27 more share classes (truncated)

(fund.ensure-umbrella
  :name "Allianz US High Yield"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_us_high_yield_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_ie
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Treasury Short Term Plus Euro"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_treasury_short__8182c8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__8182c8
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz China Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_china_equity_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_ie
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Europe Small Cap Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_europe_small_ca_402f4c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_402f4c
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Euro"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__c609d9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__c609d9
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz GEM Equity High Dividend"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_gem_equity_high_f0f7bf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_f0f7bf
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz European Equity Dividend"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_european_equity_ca00b5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_ca00b5
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Global Diversified Credit"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_f4b8c1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_f4b8c1
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Euroland Equity Growth"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_euroland_equity_69a1f7)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_69a1f7
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Best Styles Euroland Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_176318)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_176318
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz US Equity Fund"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_fund_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_ie
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz US Short Duration High Income Bond"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_us_short_durati_007ed5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_007ed5
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Global Opportunistic Bond"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_opportun_e14702)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_e14702
  :role "FUND")































(fund.ensure-umbrella
  :name "Allianz Multi Asset Long / Short"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_lon_57b07b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_57b07b
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Global High Yield"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_high_yield_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_ie
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Euro High Yield Bond"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_284d68)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_284d68
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Short Duration"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__dbd4d0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__dbd4d0
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 75"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_bf46ec)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_bf46ec
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Dynamic Commodities"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_commodities_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_ie
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Emerging Markets Short Duration Bond"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_47e88b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_47e88b
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 50"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_181c32)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_181c32
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 15"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_b2dbc2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_b2dbc2
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth Select"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_9b7761)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_9b7761
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Global Metals and Mining"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_metals_a_397d82)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_397d82
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Strategy Select 50"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_50_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_50_ie
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz German Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_german_equity_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_ie
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global Sustainability"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_sustaina_f50b5f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_f50b5f
  :role "FUND")



































(fund.ensure-subfund
  :name "Allianz Euro Cash"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_euro_cash_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_cash_ie
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Equity Unconstrained"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_u_7cb91c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_7cb91c
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Total Return Asian Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_total_return_as_4542a5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_4542a5
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Asia Ex China Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_asia_ex_china_e_10c0d9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_ex_china_e_10c0d9
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Oriental Income"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_oriental_income_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_ie
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Strategy Select 75"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_75_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_75_ie
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Equity Insights"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_i_a9d138)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_a9d138
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz All China Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_all_china_equity_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_ie
  :role "FUND")





























(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity SRI"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_8bca5c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_8bca5c
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity Opportunities"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_bfd778)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_bfd778
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Strategy Select 30"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_30_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_30_ie
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz China A-Shares"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_china_a_shares_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_ie
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz Japan Smaller Companies Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_japan_smaller_c_cbe4c2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_cbe4c2
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global Bond Fund"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_bond_fund_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_bond_fund_ie
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity SRI"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_af2f4b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_af2f4b
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Emerging Markets Sovereign Bond"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_5d04e0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_5d04e0
  :role "FUND")


























(fund.ensure-umbrella
  :name "Allianz China A Opportunities"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_china_a_opportu_69cae0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_69cae0
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Emerging Europe Equity"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_emerging_europe_8c84a8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_8c84a8
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz Global Infrastructure ELTIF"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_global_infrastr_739103)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_infrastr_739103
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Food Security"
  :jurisdiction "IE"
  :regulatory-status "UCITS"
  :as @allianz_food_security_ie)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_ie
  :role "FUND")










(fund.ensure-subfund
  :name "Allianz Money Market US $"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_money_market_us_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_money_market_us_lu
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz US High Yield"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_us_high_yield_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_lu
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Best Styles Pacific Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_pac_9e0c01)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_pac_9e0c01
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Hong Kong Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_hong_kong_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hong_kong_equity_lu
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Opportunistic Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_opportun_926c0c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_926c0c
  :role "FUND")































(fund.ensure-umbrella
  :name "Allianz Green Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_green_bond_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_lu
  :role "FUND")
























(fund.ensure-umbrella
  :name "Allianz All China Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_all_china_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_lu
  :role "FUND")





























(fund.ensure-umbrella
  :name "Allianz US Investment Grade Credit"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_us_investment_g_8ff2f1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_8ff2f1
  :role "FUND")





































(fund.ensure-umbrella
  :name "Allianz High Dividend Asia Pacific Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_high_dividend_a_8b4a95)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_high_dividend_a_8b4a95
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth Select"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_79db9f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_79db9f
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Asia Pacific Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_asia_pacific_income_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_pacific_income_lu
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz China A Opportunities"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_china_a_opportu_a0750b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_a0750b
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz China A-Shares"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_china_a_shares_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_lu
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz Pet and Animal Wellbeing"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_pet_and_animal__5367bc)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pet_and_animal__5367bc
  :role "FUND")























(fund.ensure-umbrella
  :name "Allianz Total Return Asian Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_total_return_as_bd7fa8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_bd7fa8
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Emerging Markets Short Duration Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_60d1b2)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_60d1b2
  :role "FUND")











(fund.ensure-subfund
  :name "Allianz Multi Asset Risk Control"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_ris_f23424)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_ris_f23424
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Euro Inflation-linked Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_inflation__5fda91)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_inflation__5fda91
  :role "FUND")










(fund.ensure-subfund
  :name "VermögensManagement Chance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_chance_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_chance_lu
  :role "FUND")


(fund.ensure-subfund
  :name "VermögensManagement Wachstum"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_wac_afbd38)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_afbd38
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Innovation Souveraineté Européenne"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_innovation_souv_3c47ac)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_innovation_souv_3c47ac
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Europe Small Cap Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_small_ca_1e089a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_1e089a
  :role "FUND")









(fund.ensure-subfund
  :name "VermögensManagement Balance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_balance_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_balance_lu
  :role "FUND")



(fund.ensure-subfund
  :name "VermögensManagement Substanz"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_sub_fdf596)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sub_fdf596
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Europe Equity Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_g_c25e46)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_c25e46
  :role "FUND")

























(fund.ensure-umbrella
  :name "Allianz Global Equity Insights"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_i_3b4d08)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_3b4d08
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Strategy 50"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy_50_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_50_lu
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Climate Transition Europe"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_climate_transit_b1ad79)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_b1ad79
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global High Yield"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_high_yield_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_lu
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Strategy 15"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy_15_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_15_lu
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_debc0a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_debc0a
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Best Styles Europe Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_a637ed)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_a637ed
  :role "FUND")
















(fund.ensure-umbrella
  :name "IndexManagement Substanz"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @indexmanagement_substanz_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_substanz_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "IndexManagement Balance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @indexmanagement_balance_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_balance_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Strategy Select 30"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_30_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_30_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Credit"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_credit_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_lu
  :role "FUND")







(fund.ensure-umbrella
  :name "IndexManagement Wachstum"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @indexmanagement_wachstum_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_wachstum_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "IndexManagement Chance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @indexmanagement_chance_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_chance_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Emerging Markets SRI Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_3bf616)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_3bf616
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Global Dividend"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_dividend_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_dividend_lu
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Global Floating Rate Notes Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_floating_278d90)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_278d90
  :role "FUND")


















































(fund.ensure-umbrella
  :name "Allianz European Equity Dividend"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_european_equity_a7d7ea)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_a7d7ea
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz American Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_american_income_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_american_income_lu
  :role "FUND")

























(fund.ensure-umbrella
  :name "Allianz Multi Asset Long / Short"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_lon_bc099b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_bc099b
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Thematica"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_thematica_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_lu
  :role "FUND")

































(fund.ensure-umbrella
  :name "Allianz Cyber Security"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_cyber_security_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_lu
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Flexi Asia Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_flexi_asia_bond_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_asia_bond_lu
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Convertible Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_convertible_bond_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_convertible_bond_lu
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Emerging Markets Sovereign Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_b8fe7f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_b8fe7f
  :role "FUND")


























(fund.ensure-umbrella
  :name "Allianz Euro High Yield Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_ab4be9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_ab4be9
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Emerging Markets Corporate Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_6224bc)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_6224bc
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Credit Opportunities"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_credit_opportun_537426)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_537426
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Balanced Income and Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_balanced_income_4b3074)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_balanced_income_4b3074
  :role "FUND")



















(fund.ensure-umbrella
  :name "Allianz SDG Global Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_sdg_global_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_global_equity_lu
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Oriental Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_oriental_income_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_lu
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Euro"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__40e40d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__40e40d
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Global Artificial Intelligence"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_artifici_5e5f07)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_5e5f07
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Multi Asset Future"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_multi_asset_future_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_future_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Short Duration"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__ab290b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__ab290b
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Global Diversified Dividend"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_cf72df)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_cf72df
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Global Capital Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_capital_plus_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_capital_plus_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Credit Opportunities Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_credit_opportun_eecc23)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_eecc23
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Better World Defensive"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_better_world_de_963f32)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_de_963f32
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Euroland Equity Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euroland_equity_032322)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_032322
  :role "FUND")

















(fund.ensure-umbrella
  :name "Allianz Income and Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_income_and_growth_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_lu
  :role "FUND")



















































;; ... and 27 more share classes (truncated)

(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 75"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_688c3a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_688c3a
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Better World Moderate"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_better_world_mo_c12665)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_mo_c12665
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Global"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__d7cce4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__d7cce4
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Sustainability"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_sustaina_7a9db1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_7a9db1
  :role "FUND")



































(fund.ensure-umbrella
  :name "Allianz Asian Multi Income Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_asian_multi_inc_2bfebf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_multi_inc_2bfebf
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Little Dragons"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_little_dragons_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_little_dragons_lu
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz SDG Euro Credit"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_sdg_euro_credit_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_euro_credit_lu
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Global Intelligent Cities Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_intellig_ef3890)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_intellig_ef3890
  :role "FUND")























(fund.ensure-umbrella
  :name "Allianz Dynamic Commodities"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_commodities_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_lu
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz Emerging Markets Select Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_9b0122)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_9b0122
  :role "FUND")

















(fund.ensure-subfund
  :name "VermögensManagement Wachstumsländer Balance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_wac_374f68)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_374f68
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz US Large Cap Value"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_us_large_cap_value_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_lu
  :role "FUND")















(fund.ensure-subfund
  :name "money mate entschlossen"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @money_mate_entschlossen_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_entschlossen_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Renminbi Fixed Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_renminbi_fixed__3100a8)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_renminbi_fixed__3100a8
  :role "FUND")








(fund.ensure-subfund
  :name "money mate mutig"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @money_mate_mutig_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_mutig_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Best Styles US Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__c543dd)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__c543dd
  :role "FUND")






















(fund.ensure-subfund
  :name "money mate moderat"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @money_mate_moderat_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_moderat_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Smart Energy"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_smart_energy_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_smart_energy_lu
  :role "FUND")















(fund.ensure-subfund
  :name "money mate defensiv"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @money_mate_defensiv_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_defensiv_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Better World Dynamic"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_better_world_dy_b3dbc1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_dy_b3dbc1
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz European Micro Cap"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_european_micro_cap_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_micro_cap_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Europe Equity SRI"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_sri_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_sri_lu
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz Global Metals and Mining"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_metals_a_c48267)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_c48267
  :role "FUND")










(fund.ensure-subfund
  :name "VermögensManagement RenditeStars"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_ren_307787)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_307787
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Capital Plus Global"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_capital_plus_global_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_global_lu
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Diversified Credit"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_diversif_8d84ec)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_8d84ec
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz Euro Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_bond_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_lu
  :role "FUND")














(fund.ensure-subfund
  :name "VermögensManagement AktienStars"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_akt_6a5a4d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_akt_6a5a4d
  :role "FUND")




(fund.ensure-subfund
  :name "MetallRente FONDS PORTFOLIO"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @metallrente_fonds_portfolio_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @metallrente_fonds_portfolio_lu
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Dynamic Asian High Yield Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_asian_h_18f5d9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_18f5d9
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz European Bond RC"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_european_bond_rc_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_bond_rc_lu
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond II"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_f6b7e9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_f6b7e9
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz Euro Cash"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_cash_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_cash_lu
  :role "FUND")






(fund.ensure-subfund
  :name "VermögensManagement Einkommen Europa"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_ein_a97f62)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ein_a97f62
  :role "FUND")


(fund.ensure-subfund
  :name "AEVN CDO - Cofonds"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @aevn_cdo_cofonds_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @aevn_cdo_cofonds_lu
  :role "FUND")


(fund.ensure-subfund
  :name "Allianz Stiftungsfonds"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_stiftungsfonds_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_stiftungsfonds_lu
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Europe Equity powered by Artificial Intelligence"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_p_397d35)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_p_397d35
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz US Equity powered by Artificial Intelligence"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_power_15c017)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_power_15c017
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Water"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_water_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_lu
  :role "FUND")
































(fund.ensure-umbrella
  :name "Allianz Strategic Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategic_bond_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_lu
  :role "FUND")



























(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 50"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_9736eb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_9736eb
  :role "FUND")




























(fund.ensure-umbrella
  :name "Allianz Global Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_income_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_income_lu
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Premium Champions"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_premium_champions_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_premium_champions_lu
  :role "FUND")





(fund.ensure-subfund
  :name "SK Welt"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @sk_welt_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_welt_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Asia Ex China Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_asia_ex_china_e_de39f0)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_ex_china_e_de39f0
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Global Equity powered by Artificial Intelligence"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_p_dc2b58)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_p_dc2b58
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_cd6612)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_cd6612
  :role "FUND")






























(fund.ensure-umbrella
  :name "Allianz Select Income and Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_select_income_a_0cc378)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_select_income_a_0cc378
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond III"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_f2e0a9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_f2e0a9
  :role "FUND")








(fund.ensure-subfund
  :name "ALLIANZ SECURICASH SRI"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_securicash_sri_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_securicash_sri_lu
  :role "FUND")




(fund.ensure-subfund
  :name "SK Themen"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @sk_themen_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_themen_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz US Short Duration High Income Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_us_short_durati_8debc5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_8debc5
  :role "FUND")











































(fund.ensure-umbrella
  :name "Allianz Strategy Select 75"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_75_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_75_lu
  :role "FUND")



(fund.ensure-subfund
  :name "SK Europa"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @sk_europa_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_europa_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Strategy Select 50"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy_select_50_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_50_lu
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Europe Equity Value"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_equity_value_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_value_lu
  :role "FUND")







(fund.ensure-subfund
  :name "Allianz FinanzPlan 2050"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2050_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2050_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Global Equity Unconstrained"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_u_46ff3c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_46ff3c
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Treasury Short Term Plus Euro"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_treasury_short__787732)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__787732
  :role "FUND")








(fund.ensure-subfund
  :name "Allianz Strategiefonds Balance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategiefonds__d7ae0d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__d7ae0d
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Alternative Investment Strategies"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_alternative_inv_48558e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_alternative_inv_48558e
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Allocation Opportunities"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_allocati_151f62)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_allocati_151f62
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Volatility Strategy Fund"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_volatility_stra_e374ea)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_e374ea
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz SRI Multi Asset 75"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_sri_multi_asset_75_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sri_multi_asset_75_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Japan Smaller Companies Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_japan_smaller_c_3d9028)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_3d9028
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz HKD Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_hkd_income_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hkd_income_lu
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Global Small Cap Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_small_ca_fa80ef)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_fa80ef
  :role "FUND")













(fund.ensure-umbrella
  :name "Allianz Europe Small and Micro Cap Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_europe_small_an_f1d987)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_an_f1d987
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Climate Transition Credit"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_climate_transit_d74538)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_d74538
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Global Bond Fund"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_bond_fund_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_bond_fund_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz GEM Equity High Dividend"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_gem_equity_high_f28051)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_f28051
  :role "FUND")





















(fund.ensure-umbrella
  :name "Allianz ActiveInvest Defensive"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_activeinvest_de_2780c5)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_activeinvest_de_2780c5
  :role "FUND")



(fund.ensure-subfund
  :name "VermögensManagement RentenStars"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_ren_d9acbd)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_d9acbd
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Euro High Yield Defensive"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_high_yield_3f71cf)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_3f71cf
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz ActiveInvest Balanced"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_activeinvest_ba_c94382)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_activeinvest_ba_c94382
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond IV"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_da5ec6)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_da5ec6
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Strategy4Life Europe 40"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy4life_e_f65b65)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy4life_e_f65b65
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Asian Small Cap Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_asian_small_cap_921fd9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_small_cap_921fd9
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Euro Credit SRI"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_credit_sri_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_lu
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz ActiveInvest Dynamic"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_activeinvest_dy_db4e9a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_activeinvest_dy_db4e9a
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz UK Government Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_uk_government_bond_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_government_bond_lu
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Euro Government Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_government_251b71)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_government_251b71
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Global Equity Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_equity_g_4739ef)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_g_4739ef
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 15"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_307efb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_307efb
  :role "FUND")




















(fund.ensure-umbrella
  :name "Allianz German Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_german_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_lu
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Euro Balanced"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_balanced_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_balanced_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz German Small and Micro Cap"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_german_small_an_4c7809)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_small_an_4c7809
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Japan Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_japan_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_lu
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Emerging Europe Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_europe_fdeb3a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_fdeb3a
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Best Styles Europe Equity SRI"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_eb3e3d)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_eb3e3d
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz Strategie 2036 Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategie_2036_plus_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategie_2036_plus_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz China Future Technologies"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_china_future_te_acb761)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_future_te_acb761
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Dynamic Multi Asset Strategy SRI 30"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_multi_a_ce6ad1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_ce6ad1
  :role "FUND")

















(fund.ensure-subfund
  :name "Allianz Euro Oblig Court Terme ISR"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_oblig_cour_0a1070)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_oblig_cour_0a1070
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz LJ Risk Control Fund USD FCP-FIS"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_lj_risk_control_fcadbb)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_lj_risk_control_fcadbb
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Clean Planet"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_clean_planet_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_clean_planet_lu
  :role "FUND")








(fund.ensure-subfund
  :name "Anlagestruktur 1"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @anlagestruktur_1_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @anlagestruktur_1_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Advanced Fixed Income Global Aggregate"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__436165)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__436165
  :role "FUND")








(fund.ensure-umbrella
  :name "Allianz India Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_india_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_lu
  :role "FUND")
















(fund.ensure-subfund
  :name "Allianz Defensive Mix FCP-FIS"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_defensive_mix_f_52461b)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_defensive_mix_f_52461b
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Best Styles Global Equity SRI"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_b518e9)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_b518e9
  :role "FUND")


















(fund.ensure-umbrella
  :name "Allianz Floating Rate Notes Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_floating_rate_n_15b6de)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_15b6de
  :role "FUND")















(fund.ensure-umbrella
  :name "Allianz Food Security"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_food_security_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_lu
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Best Styles Global AC Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_glo_8c275c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_8c275c
  :role "FUND")






(fund.ensure-umbrella
  :name "Allianz Best Styles Euroland Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_eur_c72782)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_c72782
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Strategy 75"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_strategy_75_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_75_lu
  :role "FUND")









(fund.ensure-umbrella
  :name "Allianz AI Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_ai_income_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_ai_income_lu
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Enhanced Short Term Euro"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_enhanced_short__d34227)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_enhanced_short__d34227
  :role "FUND")














(fund.ensure-subfund
  :name "Allianz Global Infrastructure ELTIF"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_infrastr_37a504)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_infrastr_37a504
  :role "FUND")











(fund.ensure-umbrella
  :name "Allianz Trend and Brands"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_trend_and_brands_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_trend_and_brands_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Selection Small and Mid Cap Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_selection_small_2ca2ef)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_selection_small_2ca2ef
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Euro Bond Short Term 1-3 Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_euro_bond_short_bf0c20)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_short_bf0c20
  :role "FUND")




(fund.ensure-subfund
  :name "Allianz Advanced Fixed Income Euro Aggregate"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_advanced_fixed__b152b4)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__b152b4
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Global Government Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_governme_f24277)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_governme_f24277
  :role "FUND")



(fund.ensure-subfund
  :name "Flexible Portfolio"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @flexible_portfolio_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @flexible_portfolio_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Positive Change"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_positive_change_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_positive_change_lu
  :role "FUND")










(fund.ensure-umbrella
  :name "Allianz Global Hi-Tech Growth"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_hi_tech__9ee375)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__9ee375
  :role "FUND")





(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity SRI"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_6baf98)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_6baf98
  :role "FUND")












(fund.ensure-umbrella
  :name "Allianz Global Aggregate Bond"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_global_aggregat_ad825f)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_aggregat_ad825f
  :role "FUND")




(fund.ensure-umbrella
  :name "Allianz Selection Alternative"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_selection_alter_388134)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_selection_alter_388134
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Emerging Markets Equity Opportunities"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_emerging_market_25219e)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_25219e
  :role "FUND")



(fund.ensure-subfund
  :name "Best-in-One"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @best_in_one_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @best_in_one_lu
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz Selection Fixed Income"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_selection_fixed_09949a)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_selection_fixed_09949a
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Dynamic Allocation Plus Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_dynamic_allocat_7aa9ca)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_allocat_7aa9ca
  :role "FUND")









(fund.ensure-subfund
  :name "Allianz PIMCO High Yield Income Fund"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_pimco_high_yiel_0f1ee1)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pimco_high_yiel_0f1ee1
  :role "FUND")


(fund.ensure-umbrella
  :name "Allianz China Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_china_equity_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_lu
  :role "FUND")
















(fund.ensure-umbrella
  :name "Allianz Best Styles US Small Cap Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_best_styles_us__f1a666)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__f1a666
  :role "FUND")














(fund.ensure-umbrella
  :name "Allianz Systematic Enhanced US Equity"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_systematic_enha_b68545)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_systematic_enha_b68545
  :role "FUND")







(fund.ensure-umbrella
  :name "Allianz Capital Plus"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_capital_plus_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_lu
  :role "FUND")






(fund.ensure-subfund
  :name "Allianz FinanzPlan 2040"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2040_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2040_lu
  :role "FUND")



(fund.ensure-subfund
  :name "PremiumMandat Balance"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @premiummandat_balance_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_balance_lu
  :role "FUND")



(fund.ensure-subfund
  :name "VermögensManagement DividendenStars"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @verm_gensmanagement_div_ffee7c)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_div_ffee7c
  :role "FUND")





(fund.ensure-subfund
  :name "Allianz FinanzPlan 2045"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2045_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2045_lu
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz FinanzPlan 2025"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2025_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2025_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz European Autonomy"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_european_autonomy_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_autonomy_lu
  :role "FUND")










(fund.ensure-subfund
  :name "PremiumMandat Dynamik"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @premiummandat_dynamik_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_dynamik_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz US Equity Fund"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_us_equity_fund_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_lu
  :role "FUND")













(fund.ensure-subfund
  :name "Allianz FinanzPlan 2030"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2030_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2030_lu
  :role "FUND")



(fund.ensure-umbrella
  :name "Allianz Target Maturity Euro Bond I"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_target_maturity_d55c47)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_d55c47
  :role "FUND")



(fund.ensure-subfund
  :name "Allianz FinanzPlan 2035"
  :jurisdiction "LU"
  :regulatory-status "UCITS"
  :as @allianz_finanzplan_2035_lu)

(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2035_lu
  :role "FUND")



;; ======================================================================
;; ETL SUMMARY
;; ======================================================================
;; Ownership chain entities: 2
;; Subsidiaries: 2
;; Funds created: 671
;; Share classes created: 6560
;; ======================================================================