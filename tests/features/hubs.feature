Feature: Create hubs

  Scenario: A user wants to create a hub
    Given wirc is running on localhost
    When the authenticated user attempts to create a new hub
    Then the server should respond with the OK status