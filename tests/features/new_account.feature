Feature: Create an account

  Scenario: An authenticated user wants to create a new account
    Given the server is running on localhost
    When the user attempts to create a new account
    Then the user should receive account information
