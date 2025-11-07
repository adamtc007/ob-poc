package ir

import (
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"time"
)

var (
	reUUID        = regexp.MustCompile(`(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`)
	reVersion     = regexp.MustCompile(`^1\.\d+\.\d+$`)
	reCurrency    = regexp.MustCompile(`^[A-Z]{3}$`)
	reAttrID      = regexp.MustCompile(`^[A-Z]+(\.[A-Z0-9_\[\]\-]+)*$`)
	reIdempotency = regexp.MustCompile(`^[A-Za-z0-9_-]{10,128}$`)
)

// ---------- Public API ----------

func ParsePlan(data []byte) (*Plan, error) {
	var p Plan
	if err := json.Unmarshal(data, &p); err != nil {
		return nil, err
	}
	return &p, nil
}

func (p *Plan) Validate() error {
	if !reVersion.MatchString(p.Version) {
		return fmt.Errorf("version must match %q", reVersion.String())
	}
	if !reUUID.MatchString(p.PlanID) {
		return errors.New("plan_id must be a UUID string")
	}
	if len(p.Steps) == 0 {
		return errors.New("steps must be non-empty")
	}
	// created_at must be valid RFC3339 (json already parsed into time.Time)
	_ = p.CreatedAt

	for i := range p.Steps {
		if err := p.Steps[i].validate(); err != nil {
			return fmt.Errorf("step %d (%s): %w", i, p.Steps[i].Op, err)
		}
	}
	return nil
}

// ---------- Step validation & decoding ----------

func (s *Step) validate() error {
	if s.IdempotencyKey != nil && !reIdempotency.MatchString(*s.IdempotencyKey) {
		return fmt.Errorf("idempotency_key must match %q", reIdempotency.String())
	}
	switch s.Op {
	case OpInvestorStartOpportunity:
		var a InvestorStartOpportunityArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		// legal_name, domicile: string or AttrRef
		if err := requireStringOrAttrRef("legal_name", a.LegalName); err != nil {
			return err
		}
		if a.Type != "PROPER_PERSON" && a.Type != "CORPORATE" && a.Type != "TRUST" && a.Type != "FOHF" && a.Type != "NOMINEE" {
			return errors.New("type invalid")
		}
		if err := requireStringOrAttrRef("domicile", a.Domicile); err != nil {
			return err
		}
		if len(a.LEI) > 0 {
			if err := allowStringOrAttrRef("lei", a.LEI); err != nil {
				return err
			}
		}
		return nil

	case OpInvestorRecordIndication:
		var a InvestorRecordIndicationArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if !reUUID.MatchString(a.FundID) {
			return errors.New("fund_id must be UUID")
		}
		if !reUUID.MatchString(a.ClassID) {
			return errors.New("class_id must be UUID")
		}
		if a.Ticket < 0 {
			return errors.New("ticket must be >= 0")
		}
		if a.Currency != nil && !reCurrency.MatchString(*a.Currency) {
			return fmt.Errorf("currency must match %q", reCurrency.String())
		}
		if a.IndicationDate != nil {
			if err := validateDate(*a.IndicationDate); err != nil {
				return fmt.Errorf("indication_date: %w", err)
			}
		}
		return nil

	case OpKYCBegin:
		var a KYCBeginArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.RiskRating != nil {
			switch *a.RiskRating {
			case "LOW", "MEDIUM", "HIGH":
			default:
				return errors.New("risk_rating must be LOW|MEDIUM|HIGH")
			}
		}
		return nil

	case OpKYCCollectDoc:
		var a KYCCollectDocArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.DocType == "" {
			return errors.New("doc_type required")
		}
		if a.SubjectID != nil && !reUUID.MatchString(*a.SubjectID) {
			return errors.New("subject_id must be UUID")
		}
		if a.IssuedOn != nil {
			if err := validateDate(*a.IssuedOn); err != nil {
				return fmt.Errorf("issued_on: %w", err)
			}
		}
		if a.ExpiresOn != nil {
			if err := validateDate(*a.ExpiresOn); err != nil {
				return fmt.Errorf("expires_on: %w", err)
			}
		}
		return nil

	case OpKYCScreen:
		var a KYCScreenArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.Provider == "" {
			return errors.New("provider required")
		}
		if a.ScreeningDate != nil {
			if err := validateDate(*a.ScreeningDate); err != nil {
				return fmt.Errorf("screening_date: %w", err)
			}
		}
		return nil

	case OpKYCApprove:
		var a KYCApproveArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		switch a.Risk {
		case "LOW", "MEDIUM", "HIGH":
		default:
			return errors.New("risk must be LOW|MEDIUM|HIGH")
		}
		if err := validateDate(a.RefreshDue); err != nil {
			return fmt.Errorf("refresh_due: %w", err)
		}
		return nil

	case OpTaxCapture:
		var a TaxCaptureArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.FATCA == "" || a.CRS == "" || a.Form == "" {
			return errors.New("fatca, crs, form required")
		}
		if a.WithholdingRate != nil && (*a.WithholdingRate < 0 || *a.WithholdingRate > 100) {
			return errors.New("withholding_rate must be between 0 and 100")
		}
		if a.FormSignedDate != nil {
			if err := validateDate(*a.FormSignedDate); err != nil {
				return fmt.Errorf("form_signed_date: %w", err)
			}
		}
		if a.FormExpiresDate != nil {
			if err := validateDate(*a.FormExpiresDate); err != nil {
				return fmt.Errorf("form_expires_date: %w", err)
			}
		}
		return nil

	case OpBankSetInstruction:
		var a BankSetInstructionArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if !reCurrency.MatchString(a.Currency) {
			return fmt.Errorf("currency must match %q", reCurrency.String())
		}
		if a.ActiveFrom != nil {
			if err := validateDate(*a.ActiveFrom); err != nil {
				return fmt.Errorf("active_from: %w", err)
			}
		}
		if a.ActiveTo != nil {
			if err := validateDate(*a.ActiveTo); err != nil {
				return fmt.Errorf("active_to: %w", err)
			}
		}
		if a.SWIFTBIC == "" {
			return errors.New("swift_bic required")
		}
		return nil

	case OpSubscribeRequest:
		var a SubscribeRequestArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if !reUUID.MatchString(a.ClassID) {
			return errors.New("class_id must be UUID")
		}
		if a.SeriesID != nil && !reUUID.MatchString(*a.SeriesID) {
			return errors.New("series_id must be UUID")
		}
		if a.Amount < 0 {
			return errors.New("amount must be >= 0")
		}
		if !reCurrency.MatchString(a.Currency) {
			return fmt.Errorf("currency must match %q", reCurrency.String())
		}
		if err := validateDate(a.TradeDate); err != nil {
			return fmt.Errorf("trade_date: %w", err)
		}
		if a.BankID != nil && !reUUID.MatchString(*a.BankID) {
			return errors.New("bank_id must be UUID")
		}
		return nil

	case OpCashConfirm:
		var a CashConfirmArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.TradeID != nil && !reUUID.MatchString(*a.TradeID) {
			return errors.New("trade_id must be UUID")
		}
		if a.Amount < 0 {
			return errors.New("amount must be >= 0")
		}
		if err := validateDate(a.ValueDate); err != nil {
			return fmt.Errorf("value_date: %w", err)
		}
		if a.BankID != nil && !reUUID.MatchString(*a.BankID) {
			return errors.New("bank_id must be UUID")
		}
		return nil

	case OpDealNAV:
		var a DealNAVArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.FundID) {
			return errors.New("fund_id must be UUID")
		}
		if a.ClassID != nil && !reUUID.MatchString(*a.ClassID) {
			return errors.New("class_id must be UUID")
		}
		if err := validateDate(a.NAVDate); err != nil {
			return fmt.Errorf("nav_date: %w", err)
		}
		if a.NAVPerShare != nil && *a.NAVPerShare <= 0 {
			return errors.New("nav_per_share must be > 0")
		}
		if a.SharesOutstanding != nil && *a.SharesOutstanding < 0 {
			return errors.New("shares_outstanding must be >= 0")
		}
		return nil

	case OpSubscribeIssue:
		var a SubscribeIssueArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.TradeID != nil && !reUUID.MatchString(*a.TradeID) {
			return errors.New("trade_id must be UUID")
		}
		if !reUUID.MatchString(a.ClassID) {
			return errors.New("class_id must be UUID")
		}
		if a.SeriesID != nil && !reUUID.MatchString(*a.SeriesID) {
			return errors.New("series_id must be UUID")
		}
		if a.NAVPerShare <= 0 {
			return errors.New("nav_per_share must be > 0")
		}
		if a.Units <= 0 {
			return errors.New("units must be > 0")
		}
		if a.NAVDate != nil {
			if err := validateDate(*a.NAVDate); err != nil {
				return fmt.Errorf("nav_date: %w", err)
			}
		}
		if a.SettlementDate != nil {
			if err := validateDate(*a.SettlementDate); err != nil {
				return fmt.Errorf("settlement_date: %w", err)
			}
		}
		if a.FeesAmount != nil && *a.FeesAmount < 0 {
			return errors.New("fees_amount must be >= 0")
		}
		return nil

	case OpKYCRefreshSchedule:
		var a KYCRefreshScheduleArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.Frequency == "" {
			return errors.New("frequency required")
		}
		switch a.Frequency {
		case "ANNUAL", "BIANNUAL", "QUARTERLY", "MONTHLY":
		default:
			return errors.New("frequency must be ANNUAL|BIANNUAL|QUARTERLY|MONTHLY")
		}
		if err := validateDate(a.Next); err != nil {
			return fmt.Errorf("next: %w", err)
		}
		return nil

	case OpScreenContinuous:
		var a ScreenContinuousArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.Frequency == "" {
			return errors.New("frequency required")
		}
		switch a.Frequency {
		case "DAILY", "WEEKLY", "MONTHLY":
		default:
			return errors.New("frequency must be DAILY|WEEKLY|MONTHLY")
		}
		return nil

	case OpRedeemRequest:
		var a RedeemRequestArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if !reUUID.MatchString(a.ClassID) {
			return errors.New("class_id must be UUID")
		}
		if a.SeriesID != nil && !reUUID.MatchString(*a.SeriesID) {
			return errors.New("series_id must be UUID")
		}
		if a.Units <= 0 {
			return errors.New("units must be > 0")
		}
		if a.Percentage != nil && (*a.Percentage <= 0 || *a.Percentage > 100) {
			return errors.New("percentage must be between 0 and 100")
		}
		if err := validateDate(a.NoticeDate); err != nil {
			return fmt.Errorf("notice_date: %w", err)
		}
		if a.TradeDate != nil {
			if err := validateDate(*a.TradeDate); err != nil {
				return fmt.Errorf("trade_date: %w", err)
			}
		}
		if a.BankID != nil && !reUUID.MatchString(*a.BankID) {
			return errors.New("bank_id must be UUID")
		}
		return nil

	case OpRedeemSettle:
		var a RedeemSettleArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.TradeID != nil && !reUUID.MatchString(*a.TradeID) {
			return errors.New("trade_id must be UUID")
		}
		if a.Amount < 0 {
			return errors.New("amount must be >= 0")
		}
		if err := validateDate(a.SettleDate); err != nil {
			return fmt.Errorf("settle_date: %w", err)
		}
		if a.BankID != nil && !reUUID.MatchString(*a.BankID) {
			return errors.New("bank_id must be UUID")
		}
		if a.FXRate != nil && *a.FXRate <= 0 {
			return errors.New("fx_rate must be > 0")
		}
		if a.FeesAmount != nil && *a.FeesAmount < 0 {
			return errors.New("fees_amount must be >= 0")
		}
		return nil

	case OpOffboardClose:
		var a OffboardCloseArgs
		if err := json.Unmarshal(s.ArgsRaw, &a); err != nil {
			return err
		}
		if !reUUID.MatchString(a.InvestorID) {
			return errors.New("investor_id must be UUID")
		}
		if a.ClosureDate != nil {
			if err := validateDate(*a.ClosureDate); err != nil {
				return fmt.Errorf("closure_date: %w", err)
			}
		}
		if a.Reason != nil {
			switch *a.Reason {
			case "VOLUNTARY", "INVOLUNTARY", "REGULATORY", "DECEASED", "MERGED":
			default:
				return errors.New("reason must be VOLUNTARY|INVOLUNTARY|REGULATORY|DECEASED|MERGED")
			}
		}
		if a.RetentionPeriodYears != nil && *a.RetentionPeriodYears < 0 {
			return errors.New("retention_period_years must be >= 0")
		}
		return nil

	default:
		return fmt.Errorf("unknown op %q", s.Op)
	}
}

// ---------- Helpers ----------

func validateDate(s string) error {
	_, err := time.Parse("2006-01-02", s)
	if err != nil {
		return errors.New("must be YYYY-MM-DD")
	}
	return nil
}

func requireStringOrAttrRef(field string, raw json.RawMessage) error {
	if err := allowStringOrAttrRef(field, raw); err != nil {
		return err
	}
	// must be present → enforced by caller having provided raw
	return nil
}

func allowStringOrAttrRef(field string, raw json.RawMessage) error {
	// Try string
	var asStr string
	if err := json.Unmarshal(raw, &asStr); err == nil && asStr != "" {
		return nil
	}
	// Try AttrRef
	var asObj map[string]any
	if err := json.Unmarshal(raw, &asObj); err == nil {
		if k, ok := asObj["kind"].(string); ok && k == "AttrRef" {
			if id, ok := asObj["id"].(string); ok && reAttrID.MatchString(id) {
				return nil
			}
			return fmt.Errorf("%s.id invalid (expected canonical ATTR ID)", field)
		}
	}
	return fmt.Errorf("%s must be string or AttrRef", field)
}

// Expose a typed decode helper if you want to branch on op in executors.
func (s *Step) DecodeArgs(v any) error {
	return json.Unmarshal(s.ArgsRaw, v)
}
