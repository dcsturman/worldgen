- Fix placement: Process Trades + Profit should be rendered after the Total row, not before
- Re-add UI row with Process Trades + Profit in the Revenue section, after the Total span
- Ensure Goods Profit row always renders when show_sell_price is true; investigate the blank box issue
- Confirm Sell Qty input logic: no mutation of manifest quantities on change; only set sell_plan
- After fixes, cargo check and run app to visually verify layout

