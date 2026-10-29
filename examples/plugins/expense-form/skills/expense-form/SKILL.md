---
name: expense-form
description: Record expenses and show the month's totals in a form the user can fill. Use when the user mentions spending money, a receipt, an expense report, or asks what they spent.
---

# Expense form

1. To show the form and this month's expenses: call
   `tools.expense_form.open_expenses` from run_typescript (`month`:
   `YYYY-MM`, optional).
2. To record one the user told you about: call
   `tools.expense_form.add_expense` with `amount`, `currency` (EUR by
   default), `category` (food, transport, lodging, office, software,
   other), `date` (`YYYY-MM-DD`, today by default) and a short `note`.
3. The user can also add expenses in the form; each one waits for their
   approval in bise first. The form keeps you posted with model context
   ("Expenses in 2026-10, as shown in the form: …"): use those totals to
   answer "how much did I spend", no tool call needed.
