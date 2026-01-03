;; Board Control Test
;; Tests board composition and appointment rights
;; Phase D.2 of KYC Control Enhancement

(cbu.create
    :name "Board Control Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Board Test Ltd"
    :company-number "UK888777"
    :jurisdiction "GB"
    :as @company)

;; Create investors with appointment rights
(entity.create-limited-company
    :cbu-id @cbu
    :name "PE Fund I LP"
    :company-number "KY12345"
    :jurisdiction "KY"
    :as @pe-fund)

;; Create directors
(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Alice"
    :last-name "Chair"
    :date-of-birth "1965-03-20"
    :nationality "GB"
    :as @alice)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Bob"
    :last-name "Director"
    :date-of-birth "1975-08-10"
    :nationality "GB"
    :as @bob)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Carol"
    :last-name "Nominee"
    :date-of-birth "1980-12-01"
    :nationality "US"
    :as @carol)

;; Grant appointment rights (from SHA)
(board.grant-appointment-right
    :cbu-id @cbu
    :target-entity-id @company
    :holder-entity-id @pe-fund
    :right-type "APPOINT_AND_REMOVE"
    :max-appointments 2
    :source-clause "Clause 5.2(a)")

;; Make appointments - Chairman
(board.appoint
    :cbu-id @cbu
    :entity-id @company
    :person-entity-id @alice
    :position "CHAIRMAN"
    :appointment-date "2020-01-15")

;; Make appointments - Executive Director
(board.appoint
    :cbu-id @cbu
    :entity-id @company
    :person-entity-id @bob
    :position "EXECUTIVE_DIRECTOR"
    :appointment-date "2020-01-15")

;; Make appointments - Nominee Director appointed by PE fund
(board.appoint
    :cbu-id @cbu
    :entity-id @company
    :person-entity-id @carol
    :position "NON_EXECUTIVE_DIRECTOR"
    :appointed-by-entity-id @pe-fund
    :appointment-date "2021-06-01")

;; Analyze board control - who controls through appointments?
(board.analyze-control
    :entity-id @company
    :as @control-analysis)

;; List all board members
(board.list-by-entity
    :entity-id @company
    :as @board-members)

;; List appointment rights held by PE fund
(board.list-rights-held
    :holder-entity-id @pe-fund
    :as @pe-rights)
