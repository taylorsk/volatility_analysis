﻿# volatility_analysis
This tool uses the free AlphaVantage API to compare the accuracy of the Historical Volatility to the accuracy of the Implied Volatility. By default (due to API limitations), it requests and calculates the Implied volatility and Historical Volatility every 2 weeks, 24 times. It will output a graph along with the MAE and Correlation values.
# Usage
Set the API key and ticker symbol in main, and run. You may also change the other settings such as the HV/IV window.
