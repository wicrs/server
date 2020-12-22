Feature: Create hubs

  Scenario: An authenticated user wants to create a hub
    Given the server is running on localhost
    Given the user has an account
    When the user attempts to create a new hub
    Then the user should receive an ID
