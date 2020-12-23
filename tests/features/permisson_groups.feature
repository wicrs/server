Feature: Permission groups

    Scenario: An authenticated user wants to know what permission groups exist in the server
        Given the user is in a hub
        When the user asks the server for a list ranks
        Then the user should receive a list of ranks along with their metadata

    Scenario: An authenticated user wants to know what permission groups they have
        Given the user is in a hub
        When the user asks the server for a list of their ranks
        Then the user should receive a list of ranks along with their metadata
