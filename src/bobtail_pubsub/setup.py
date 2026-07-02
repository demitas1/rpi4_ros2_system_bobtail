from setuptools import find_packages, setup

package_name = 'bobtail_pubsub'

setup(
    name=package_name,
    version='0.0.0',
    packages=find_packages(exclude=['test']),
    data_files=[
        ('share/ament_index/resource_index/packages',
            ['resource/' + package_name]),
        ('share/' + package_name, ['package.xml']),
    ],
    install_requires=['setuptools'],
    zip_safe=True,
    maintainer='root',
    maintainer_email='root@todo.todo',
    description='system_bobtail サンプル: talker/listener (py_pubsub 由来)',
    license='Apache-2.0',
    tests_require=['pytest'],
    entry_points={
        'console_scripts': [
                'bobtail_talker = bobtail_pubsub.publisher_member_function:main',
                'bobtail_listener = bobtail_pubsub.subscriber_member_function:main',
        ],
    },
)
