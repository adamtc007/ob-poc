# Client Portal User Manual

## Overview

The Client Portal provides a conversational interface for responding to onboarding requests. Instead of forms and email chains, you interact with an intelligent assistant that:

- Explains **what** we need and **why** (regulatory context)
- Accepts documents and information through chat
- Tracks partial progress across sessions
- Connects you to your relationship manager when needed

## Getting Started

### Accessing the Portal

1. Navigate to the client portal URL provided by your relationship manager
2. Log in with your client credentials
3. You'll see a chat interface with your onboarding status

### Your First Conversation

```
You: "Hi, what do you need from us?"

Agent: "Hello! Here's where we are with Acme Holdings:

**Overall Progress: 73% complete**

Outstanding items:

1. **Source of Wealth - Pierre Dupont** (due Jan 15)
   WHY: Pierre is investing €50M. FCA regulations require us to 
   verify how he accumulated this wealth.
   ACCEPTS: Tax returns, audited accounts, or advisor letter

2. **Identity Document - Thomas Müller** (due Jan 10)
   WHY: UK Money Laundering Regulations require ID verification 
   for all persons with significant control.
   ACCEPTS: Passport or national ID card

That's everything! Want to start with either of these?"
```

## Common Tasks

### Checking Status

**Ask:** "What's the status of our onboarding?"

**Response includes:**
- Overall progress percentage
- Completed stages
- Outstanding items with due dates
- Any overdue items flagged

---

### Understanding Why Documents Are Needed

**Ask:** "Why do you need 3 years of tax returns?"

**Response:**
```
Fair question. Here's the regulatory background:

**The requirement**: FCA SYSC 6.1.1 requires firms to verify 'source 
of wealth' for high-value clients.

**Why 3 years**: A single year's return only shows current income. 
3 years establishes a pattern that explains wealth accumulation.

**What we're looking for**: We're not auditing your finances - we're 
confirming the wealth has a legitimate, documented source.

**Alternatives**: If tax returns are problematic, we can also accept:
- Audited financial statements
- Letter from your tax advisor
- Combination of bank statements + income verification

Would any of those alternatives work better for you?
```

---

### Uploading Documents

**Method 1: Drag and drop**
- Drag a file directly into the chat window
- The agent identifies the document type and links it to the correct request

**Method 2: Explicit upload**
```
You: "I have Thomas's passport to upload"

Agent: "Great! Go ahead and drop the file here, or click the upload 
button. I'll tag it to Thomas Müller's identity verification request."

[You upload: Thomas_Mueller_Passport.pdf]

Agent: "Received Thomas Müller's passport. ✓

I've submitted it for review - you'll see it marked complete 
within 24 hours once verified."
```

---

### Providing Information Through Chat

For structured data (like beneficial owner details), the agent guides you through questions:

```
You: "We need to add a new beneficial owner - our COO"

Agent: "Sure, I can add them now. What's their full legal name?"

You: "Thomas Müller"

Agent: "Thomas Müller - got it. Is that M-Ü-L-L-E-R with an umlaut?"

You: "Yes, with the umlaut"

Agent: "What's Thomas's role at Acme Holdings?"

You: "COO, started last month"

Agent: "Does Thomas have signing authority, voting rights above 25%, 
or the right to appoint/remove board members?"

You: "Joint signing authority with the CEO"

Agent: "Got it. I'll need a few more details:
- Date of birth?
- Nationality?
- Country of tax residence?"

You: "German, lives in Munich, born 8th August 1970"

Agent: "Let me confirm what I'm recording:

**Thomas Müller**
- Role: Chief Operating Officer
- Control type: Significant control (joint signing)
- DOB: 8 August 1970
- Nationality: German
- Tax residence: Germany

Is that all correct?"

You: "Yes"

Agent: "Added. ✓ I'll need identity verification for Thomas as well.
You can upload that now or I'll add it to your outstanding items."
```

---

### Adding Notes About Expected Delivery

```
You: "The tax returns are still with our accountant"

Agent: "No problem - when do you expect to have them?"

You: "Should be next week, maybe Friday"

Agent: "I've noted that you're expecting them by Friday January 3rd.
I'll check back if we haven't received them by then. 

Is that okay?"

You: "Perfect"
```

---

### Coming Back to a Previous Session

The portal remembers your progress. When you return:

```
Agent: "Welcome back! Last time we spoke, you were getting:
- Tax returns for 2021-2022 from your accountant (expected Jan 3)
- Thomas Müller's ID from HR (expected tomorrow)

What have you got for me?"
```

---

### Requesting Help from a Person

```
You: "I need to speak to someone about this"

Agent: "Of course. I can connect you with your relationship manager 
Sarah Chen. She has full context from our conversation.

Would you prefer a call, email, or video meeting?"

You: "Email please"

Agent: "I've sent a request to Sarah with our full conversation 
history attached. You should hear back within 24 hours.

Is there anything else I can help with in the meantime?"
```

## Understanding Outstanding Requests

Each outstanding request includes:

| Field | Description |
|-------|-------------|
| **What** | The document or information needed |
| **For whom** | Which person or entity it relates to |
| **WHY** | The regulatory requirement in plain language |
| **Accepts** | What types of documents satisfy this |
| **Due date** | When we need it by |
| **Status** | Pending, Partially complete, Overdue |

### Request Statuses

| Status | Meaning |
|--------|---------|
| **Pending** | Waiting for your response |
| **Partially Complete** | Some items received, more needed |
| **In Review** | Document received, being verified |
| **Complete** | All requirements satisfied |
| **Overdue** | Past due date - please prioritize |

## Document Types We Accept

### Identity Verification
- Passport (preferred)
- National ID card
- Driver's license (some jurisdictions)

### Address Verification
- Utility bill (dated within 3 months)
- Bank statement
- Government correspondence

### Corporate Documents
- Certificate of Incorporation
- Register of Directors
- Shareholder Register
- Articles of Association

### Financial Documentation
- Audited financial statements
- Tax returns
- Bank statements
- Source of wealth documentation

## Tips for Efficient Onboarding

1. **Upload documents as you get them** - Partial progress is saved
2. **Ask "why" if unsure** - We'll explain the regulatory context
3. **Add notes about delays** - We can adjust expectations
4. **Use chat for data collection** - It's faster than forms
5. **Check back regularly** - The portal shows real-time status

## Frequently Asked Questions

### "Why do you need this document?"
Every request includes a WHY section explaining the regulatory requirement. Ask the agent to explain further if needed.

### "Can I send something else instead?"
Ask the agent about alternatives. Many requirements can be satisfied by different document types.

### "I already sent this"
Tell the agent. We'll check if it was received and linked correctly.

### "This is taking too long"
Ask to escalate. We'll connect you with your relationship manager.

### "I don't have access to this information"
Tell the agent who in your organization might have it. We can note the internal contact.

### "The due date is unrealistic"
Ask for an extension. We can adjust timelines for legitimate reasons.

## Privacy and Security

- All documents are transmitted over encrypted connections
- Access is restricted to authorized personnel
- Documents are retained per regulatory requirements
- You can request a summary of documents we hold

## Getting Help

**In the portal:** Type "I need help" or "Speak to someone"

**Email:** Your relationship manager's email is shown in the portal

**Phone:** Contact details available after login

---

## Appendix: Sample Conversations

### A. Quick Status Check

```
You: "Hi, quick check - what's still outstanding?"

Agent: "Two items remaining:
1. Maria's date of birth (quick one!)
2. Tax returns 2021-2022 (you said accountant sending Friday)

You're 94% done."
```

### B. Document Upload with Auto-Tagging

```
[You drag and drop: Acme_Holdings_Tax_2023.pdf]

Agent: "Got it - 'Acme_Holdings_Tax_2023.pdf'

This looks like a tax return for Acme Holdings. I've tagged it to 
your Source of Wealth request.

Still need: 2021 and 2022 returns. Want me to note that these are 
coming from your accountant?"
```

### C. Correcting Information

```
You: "Actually, Maria's birthday is March 15, not March 5"

Agent: "Thanks for the correction. I've updated Maria Schmidt's 
date of birth from 5 March 1972 to 15 March 1972. ✓"
```

### D. Understanding Complex Requirements

```
You: "What exactly is 'significant control'?"

Agent: "Significant control means having influence over the company 
without necessarily owning shares. This includes:

- The right to appoint or remove directors
- Significant influence over decisions
- Joint signing authority
- Veto rights on major matters

For regulatory purposes (UK PSC regime), anyone with significant 
control must be disclosed, even if they own 0% of shares.

In Thomas's case, his joint signing authority qualifies as 
significant control."
```

### E. Partial Progress

```
You: "I have some of the documents but not all"

Agent: "No problem! What do you have? I'll record what you can 
send now and we'll track the rest separately."

You: "I have 2023 tax return but still waiting on 2021 and 2022"

Agent: "Perfect. Send the 2023 return now - I'll mark your Source 
of Wealth request as 'Partially Complete (1 of 3 years)'.

Drop the file here when you're ready."
```
