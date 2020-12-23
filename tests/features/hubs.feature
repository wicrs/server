Feature: Hubs

    Scenario: An authenticated user wants to create a hub
        Given the user has an account
        When the user attempts to create a new hub
        Then the user should receive an ID

    Scenario: The owner of a hub wants to get all of it's metadata
        Given the user is in a hub
        When the user requests the hub's metadata
        Then the user should receive hub metadata

    Scenario: An authenticated user wants to get the permissions they have in a hub
        Given the user is in a hub
        When the user requests their permission data
        Then the user should receive a list of permissions

