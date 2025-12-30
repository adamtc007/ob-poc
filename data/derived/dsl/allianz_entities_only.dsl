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


(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_ch
  :role "FUND")



















































;; ... and 27 more share classes (truncated)


(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_bca879
  :role "FUND")































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_c44d45
  :role "FUND")


























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_ch
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_multi_inc_b7e1c2
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_abb706
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_50_ch
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_smart_energy_ch
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_f0ab92
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_ch
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_future_te_98f65d
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_intellig_571657
  :role "FUND")
























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_97071a
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_ch
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_ch
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_e0227f
  :role "FUND")



















































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_d37ce1
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_917243
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_5cf32a
  :role "FUND")




































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_e33a67
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_ch
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__ae11ad
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondak_ch
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_ada42c
  :role "FUND")






































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_0b8d2a
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_d5742f
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_ch
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_350665
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_government_5f58d1
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_value_ch
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_ch
  :role "FUND")


































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_ch
  :role "FUND")

































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_a471c7
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_e2fdf0
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_oblig_cour_bb34f6
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_asia_bond_ch
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_612093
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__610108
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hkd_income_ch
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_43e358
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_3b823a
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hong_kong_equity_ch
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_g_e0b93f
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_656c2e
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_small_cap_72d11a
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_94d452
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_3bae47
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_clean_planet_ch
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_48389e
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_ch
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_ch
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_ab9019
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_ch
  :role "FUND")

























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_ch
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_sri_ch
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_732bb8
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_dad7c1
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_rentenfonds_ch
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_ch
  :role "FUND")






























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_renminbi_fixed__b7214c
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_ch
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pet_and_animal__86eb53
  :role "FUND")
























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_positive_change_ch
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_74e064
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_5d4893
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_bab7cf
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__3afac6
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_ddf260
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_ch
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_nebenwerte_deut_02420e
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_b6dd78
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_allocat_5315d2
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_ch
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__92ecf1
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_verm_gensbildun_5faa5f
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_ch
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_ch
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_ch
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_af25f3
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europazins_ch
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_04a65a
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_pacific_income_ch
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_3eff39
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_valeurs_durables_ch
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_280bff
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_b00a84
  :role "FUND")
































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_22fcdb
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_convertible_bond_ch
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_8dd69a
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_euro_credit_ch
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_2ff02c
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_265b3a
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_43523c
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_ch
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_ch
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__0f9660
  :role "FUND")























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_ch
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_824fc1
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fonds_schweiz_ch
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_allocati_3e26ee
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__c1eb7a
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_41d33f
  :role "FUND")



























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__183508
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_86d083
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_319e1c
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_inflation__f45e79
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_high_dividend_a_c48a9d
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_b4a469
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_enhanced_short__80e1de
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_internationaler_3a0f27
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__5f03ae
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_diversified_swi_7818a4
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_swiss_bond_ch
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_money_market_us_de
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_de
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_pac_d697a9
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_64b000
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_de
  :role "FUND")

























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__288508
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_small_cap_fc28b4
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_de
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_de
  :role "FUND")

































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_6d30d4
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_732aa8
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_60f3c7
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_de
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_bd539e
  :role "FUND")






































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_a14f5b
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_balance_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_e9395e
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_9d8926
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hong_kong_equity_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_c2bf67
  :role "FUND")
































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_9ada8b
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_dynamik_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_5600e2
  :role "FUND")


























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_positive_change_de
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_75_de
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_alternative_inv_25ce00
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_mobil_fonds_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @concentra_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_de
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @plusfonds_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_bded97
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_de
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_manager_g_feb2cd
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategie_2036_plus_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategie_2031_plus_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_b4a933
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @anlagestruktur_1_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_de
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_cb3a69
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_inflation__5e8d9b
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_allocat_3f8418
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_high_dividend_a_f4cffe
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_706f87
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_802b05
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_g_fb4bd7
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_convertible_bond_de
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_cfe532
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_innovation_souv_0ca2e2
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_0af205
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_50_de
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_15_de
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_de
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fonds_schweiz_de
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_asia_bond_de
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_global_equity_de
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondra_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_0241ac
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondis_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_de
  :role "FUND")


































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @flexible_portfolio_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_de
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__4c7171
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_internationaler_55bf7c
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_b335db
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__a8b430
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_nebenwerte_deut_d8c4bf
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_actions_euro_co_ca2053
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_small_an_c83f30
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_governme_5f24b9
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_80f3c5
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_dividend_de
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_822620
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_aggregat_46f451
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_balanced_income_e8d1a0
  :role "FUND")




















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_de
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_de
  :role "FUND")






























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sta_80d8b7
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @convest_21_vl_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @best_in_one_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_adiverba_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_de
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pet_and_animal__3d0740
  :role "FUND")
























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pimco_high_yiel_0b4682
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_138a08
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_5e935b
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_63aacd
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_a7b52a
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_4d2c32
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_278e90
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_de
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fondsvorsorge_1_f599e3
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_multi_inc_a5f318
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_konservativ_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_de_72ac97
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_biotechnologie_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_bf00da
  :role "FUND")



























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_de
  :role "FUND")



















































;; ... and 27 more share classes (truncated)


(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_wachstum_europa_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_de
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_euro_credit_de
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_cb8d54
  :role "FUND")




































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_mo_e2d125
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__626586
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_verm_gensbildun_37d2b3
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_6225a3
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_micro_cap_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_e4dff9
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europazins_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__a27487
  :role "FUND")























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_entschlossen_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @industria_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_281227
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_mutig_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_informationstec_117281
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_rentenfonds_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_moderat_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sta_bbdd39
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_defensiv_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_substanz_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__609d3c
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_fonds_japan_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_balance_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_premium_champions_de
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_interglobal_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_verm_gensbildun_963420
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_dy_ec1947
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_renminbi_fixed__ca9577
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_wachstum_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_99f5ec
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_chance_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_6b0587
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_84e2a0
  :role "FUND")



















































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_f5b264
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_5b29bf
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_little_dragons_de
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_growth_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2050_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thesaurus_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_de
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_d_74e1e2
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__c92eca
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_global_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_rohstofffonds_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_8180a3
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_adifonds_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_54d2df
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_pacific_income_de
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_smart_energy_de
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_28251b
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_allocati_246f28
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_p_dca05c
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_ris_c20252
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_power_c72b45
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_sri_de
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_chance_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__b40bc5
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_f06600
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_p_41c25a
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_balance_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_e4f6d6
  :role "FUND")































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sub_4b4fd8
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_94b15a
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_6379c2
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_e97807
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_46e198
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__f0f85d
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_cash_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_b6c802
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy4life_e_9e9531
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_480747
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @metallrente_fonds_portfolio_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_e1cfb4
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_ex_china_e_8e9afe
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_future_te_b31d00
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ein_a7d89d
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_de
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @fondak_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_de
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_69d75a
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_stiftungsfonds_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_c39491
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_f7d45b
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_securicash_sri_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_welt_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_75_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_7cb0a4
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_themen_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_50_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_infrastr_9764e4
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_europa_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__04f2c4
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_systematic_enha_b84a95
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_value_de
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_be7392
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_de
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_intellig_296a9a
  :role "FUND")
























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__8c37a8
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hkd_income_de
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_enhanced_short__d2a16a
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_div_986aed
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_oblig_cour_887837
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_de
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_31c0bb
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__559c8a
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_d9f297
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_e90d80
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_11ccb9
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_wachstum_euroland_de
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_clean_planet_de
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_204468
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_ai_income_de
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_bond_rc_de
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_akt_57df65
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sgb_renten_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiumstars_wachstum_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiumstars_chance_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @n_rnberger_euroland_a_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @kapital_plus_de
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_rentenfonds_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_de
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_income_de
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_rentenfonds_de
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_autonomy_de
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_597e4a
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_short_62e269
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2025_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_bond_fund_de
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2030_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_eda48d
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2035_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2040_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2045_de
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_gb
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_ab96b1
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_fund_gb
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__5c41bf
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_gb
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_9fd2be
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_gb
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_2680a0
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_gb
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_listed_oppor_142517
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_gb
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_gb
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_a3d627
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_c92b69
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_36c047
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_11f841
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_bd8350
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_e22c3c
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_riskmaster_grow_6c8f61
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_5894a5
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_gb
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_gb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_riskmaster_mode_cc4414
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_gb
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_924592
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_riskmaster_cons_2ddc8a
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gilt_yield_fund_gb
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_e60b3a
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_gb
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_69f19f
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_7ee2e9
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_7ebc8b
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_continental_eur_13134a
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_government_bond_gb
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_ab5f02
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_8b44fc
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_listed_equit_6e3286
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_3a3f5b
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_00e159
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_33a53d
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_gb
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_gb
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_15_gb
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_multi_se_e9a8b8
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_gb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_b904b7
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_fund_gb
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__f051ce
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__8ed62f
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_index_linked_gi_ff9d6e
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_a1690c
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_2ea40e
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_8c7f4d
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_gb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_gb
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_dividend_gb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_d186ea
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_c281a9
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_79aeee
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares__f0834a
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_4079cc
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_technology_trus_9ddc55
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_gb
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_gb
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_531a71
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_legacy_builder_fund_gb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @the_brunner_investment__f05e68
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__759246
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_27a3d7
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @the_merchants_trust_plc_gb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_6d4cc0
  :role "FUND")


























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_ie
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_ie
  :role "FUND")



















































;; ... and 27 more share classes (truncated)


(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_ie
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__8182c8
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_ie
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_402f4c
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__c609d9
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_f0f7bf
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_ca00b5
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_f4b8c1
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_69a1f7
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_176318
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_ie
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_007ed5
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_e14702
  :role "FUND")
































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_57b07b
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_ie
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_284d68
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__dbd4d0
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_bf46ec
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_ie
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_47e88b
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_181c32
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_b2dbc2
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_9b7761
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_397d82
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_50_ie
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_ie
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_f50b5f
  :role "FUND")




































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_cash_ie
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_7cb91c
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_4542a5
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_ex_china_e_10c0d9
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_ie
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_75_ie
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_a9d138
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_ie
  :role "FUND")






























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_8bca5c
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_bfd778
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_30_ie
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_ie
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_cbe4c2
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_bond_fund_ie
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_af2f4b
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_5d04e0
  :role "FUND")



























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_69cae0
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_8c84a8
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_infrastr_739103
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_ie
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_money_market_us_lu
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_high_yield_lu
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_pac_9e0c01
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hong_kong_equity_lu
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_opportun_926c0c
  :role "FUND")
































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_green_bond_lu
  :role "FUND")

























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_all_china_equity_lu
  :role "FUND")






























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_investment_g_8ff2f1
  :role "FUND")






































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_high_dividend_a_8b4a95
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_79db9f
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_pacific_income_lu
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_opportu_a0750b
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_a_shares_lu
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pet_and_animal__5367bc
  :role "FUND")
























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_total_return_as_bd7fa8
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_60d1b2
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_ris_f23424
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_inflation__5fda91
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_chance_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_afbd38
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_innovation_souv_3c47ac
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_ca_1e089a
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_balance_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_sub_fdf596
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_g_c25e46
  :role "FUND")


























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_i_3b4d08
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_50_lu
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_b1ad79
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_high_yield_lu
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_15_lu
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_debc0a
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_a637ed
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_substanz_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_balance_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_30_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_credit_lu
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_wachstum_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @indexmanagement_chance_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_3bf616
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_dividend_lu
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_floating_278d90
  :role "FUND")



















































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_equity_a7d7ea
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_american_income_lu
  :role "FUND")


























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_lon_bc099b
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_thematica_lu
  :role "FUND")


































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_cyber_security_lu
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_flexi_asia_bond_lu
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_convertible_bond_lu
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_b8fe7f
  :role "FUND")



























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_ab4be9
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_6224bc
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_537426
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_balanced_income_4b3074
  :role "FUND")




















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_global_equity_lu
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_oriental_income_lu
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__40e40d
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_artifici_5e5f07
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_multi_asset_future_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__ab290b
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_cf72df
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_capital_plus_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_credit_opportun_eecc23
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_de_963f32
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euroland_equity_032322
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_income_and_growth_lu
  :role "FUND")



















































;; ... and 27 more share classes (truncated)


(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_688c3a
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_mo_c12665
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__d7cce4
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_sustaina_7a9db1
  :role "FUND")




































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_multi_inc_2bfebf
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_little_dragons_lu
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sdg_euro_credit_lu
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_intellig_ef3890
  :role "FUND")
























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_commodities_lu
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_9b0122
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_wac_374f68
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_large_cap_value_lu
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_entschlossen_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_renminbi_fixed__3100a8
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_mutig_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__c543dd
  :role "FUND")























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_moderat_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_smart_energy_lu
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @money_mate_defensiv_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_better_world_dy_b3dbc1
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_micro_cap_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_sri_lu
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_metals_a_c48267
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_307787
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_global_lu
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_diversif_8d84ec
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_lu
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_akt_6a5a4d
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @metallrente_fonds_portfolio_lu
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_asian_h_18f5d9
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_bond_rc_lu
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_f6b7e9
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_cash_lu
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ein_a97f62
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @aevn_cdo_cofonds_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_stiftungsfonds_lu
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_p_397d35
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_power_15c017
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_water_lu
  :role "FUND")

































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategic_bond_lu
  :role "FUND")




























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_9736eb
  :role "FUND")





























(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_income_lu
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_premium_champions_lu
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_welt_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asia_ex_china_e_de39f0
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_p_dc2b58
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_cd6612
  :role "FUND")































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_select_income_a_0cc378
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_f2e0a9
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_securicash_sri_lu
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_themen_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_short_durati_8debc5
  :role "FUND")












































(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_75_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @sk_europa_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_select_50_lu
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_equity_value_lu
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2050_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_u_46ff3c
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_treasury_short__787732
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategiefonds__d7ae0d
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_alternative_inv_48558e
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_allocati_151f62
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_volatility_stra_e374ea
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_sri_multi_asset_75_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_smaller_c_3d9028
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_hkd_income_lu
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_small_ca_fa80ef
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_europe_small_an_f1d987
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_climate_transit_d74538
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_bond_fund_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_gem_equity_high_f28051
  :role "FUND")






















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_activeinvest_de_2780c5
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_ren_d9acbd
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_high_yield_3f71cf
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_activeinvest_ba_c94382
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_da5ec6
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy4life_e_f65b65
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_asian_small_cap_921fd9
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_credit_sri_lu
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_activeinvest_dy_db4e9a
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_uk_government_bond_lu
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_government_251b71
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_equity_g_4739ef
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_307efb
  :role "FUND")





















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_equity_lu
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_balanced_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_german_small_an_4c7809
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_japan_equity_lu
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_europe_fdeb3a
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_eb3e3d
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategie_2036_plus_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_future_te_acb761
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_multi_a_ce6ad1
  :role "FUND")


















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_oblig_cour_0a1070
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_lj_risk_control_fcadbb
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_clean_planet_lu
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @anlagestruktur_1_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__436165
  :role "FUND")









(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_india_equity_lu
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_defensive_mix_f_52461b
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_b518e9
  :role "FUND")



















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_floating_rate_n_15b6de
  :role "FUND")
















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_food_security_lu
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_glo_8c275c
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_eur_c72782
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_strategy_75_lu
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_ai_income_lu
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_enhanced_short__d34227
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_infrastr_37a504
  :role "FUND")












(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_trend_and_brands_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_selection_small_2ca2ef
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_euro_bond_short_bf0c20
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_advanced_fixed__b152b4
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_governme_f24277
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @flexible_portfolio_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_positive_change_lu
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_hi_tech__9ee375
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_6baf98
  :role "FUND")













(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_global_aggregat_ad825f
  :role "FUND")





(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_selection_alter_388134
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_emerging_market_25219e
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @best_in_one_lu
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_selection_fixed_09949a
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_dynamic_allocat_7aa9ca
  :role "FUND")










(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_pimco_high_yiel_0f1ee1
  :role "FUND")



(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_china_equity_lu
  :role "FUND")

















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_best_styles_us__f1a666
  :role "FUND")















(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_systematic_enha_b68545
  :role "FUND")








(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_capital_plus_lu
  :role "FUND")







(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2040_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_balance_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @verm_gensmanagement_div_ffee7c
  :role "FUND")






(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2045_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2025_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_european_autonomy_lu
  :role "FUND")











(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @premiummandat_dynamik_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_us_equity_fund_lu
  :role "FUND")














(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_finanzplan_2030_lu
  :role "FUND")




(cbu.assign-role
  :cbu-id @cbu_agi
  :entity-id @allianz_target_maturity_d55c47
  :role "FUND")




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