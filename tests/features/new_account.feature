Feature: Create an account

  Scenario: A user wants to create a new account
    Given wirc is running on localhost
    When the authenticated user attempts to create a new account
    Then the user should receive account information