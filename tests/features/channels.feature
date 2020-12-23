Feature: Text channels

  Scenario: An authenticated user wants to create a text channel
    Given the server is running on localhost
    Given the user has an account
    Given the user is in a hub
    When the user attempts to create a new text channel
    Then the user should receive an ID

  Scenario: An authenticated user wants to know what text channels they have access to
    Given the server is running on localhost
    Given the user has an account
    Given the user is in a hub
    Given the user has access to a text channel
    When the user asks the server for a list of text channels
    Then the user should receive a list of text channels

  Scenario: An authenticated user wants to delete a text channel
    Given the server is running on localhost
    Given the user has an account
    Given the user is in a hub
    Given the user has access to a text channel
    When the user attempts to delete the text channel
    Then the user should receive the OK response
